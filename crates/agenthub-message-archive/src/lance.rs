use std::sync::Arc;

use anyhow::Result;
use arrow_array::{
    Array, ArrayRef, Float32Array, Int64Array, RecordBatch, StringArray, UInt32Array,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use async_trait::async_trait;
use futures::TryStreamExt;
use lance_index::scalar::FullTextSearchQuery;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::{Connection, connect, database::CreateTableMode, index::Index};

use crate::model::{
    MessageArchiveConfig, MessageArchiveStore, MessageDocument, MessageDocumentKind,
    MessageSearchHit, MessageSearchQuery,
};

pub struct LanceDbMessageArchive {
    connection: Connection,
    config: MessageArchiveConfig,
}

impl LanceDbMessageArchive {
    pub async fn connect(config: MessageArchiveConfig) -> Result<Self> {
        let connection = connect(&config.uri).execute().await?;
        Ok(Self { connection, config })
    }

    fn schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("document_id", DataType::Utf8, false),
            Field::new("source_kind", DataType::Utf8, false),
            Field::new("source_id", DataType::Utf8, false),
            Field::new("logical_message_id", DataType::Utf8, true),
            Field::new("team_id", DataType::Utf8, true),
            Field::new("run_id", DataType::Utf8, true),
            Field::new("conversation_id", DataType::Utf8, true),
            Field::new("task_id", DataType::Utf8, true),
            Field::new("agent_id", DataType::Utf8, true),
            Field::new("session_id", DataType::Utf8, true),
            Field::new("body_text", DataType::Utf8, false),
            Field::new("payload_json", DataType::Utf8, true),
            Field::new("created_at", DataType::Int64, false),
            Field::new("event_id_from", DataType::Int64, true),
            Field::new("event_id_to", DataType::Int64, true),
            Field::new("chunk_count", DataType::UInt32, true),
        ]))
    }

    async fn ensure_table(&self) -> Result<lancedb::Table> {
        let table = self
            .connection
            .create_empty_table(&self.config.message_table, Self::schema())
            .mode(CreateTableMode::exist_ok(|request| request))
            .execute()
            .await?;
        Ok(table)
    }

    fn documents_to_record_batch(documents: &[MessageDocument]) -> Result<RecordBatch> {
        let arrays: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| Some(doc.document_id.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| Some(doc.source_kind.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| Some(doc.source_id.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| doc.logical_message_id.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| doc.team_id.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| doc.run_id.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| doc.conversation_id.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| doc.task_id.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| doc.agent_id.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| doc.session_id.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| Some(doc.body_text.as_str()))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                documents
                    .iter()
                    .map(|doc| doc.payload_json.as_deref())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                documents
                    .iter()
                    .map(|doc| Some(doc.created_at))
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                documents
                    .iter()
                    .map(|doc| doc.event_id_from)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(
                documents
                    .iter()
                    .map(|doc| doc.event_id_to)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt32Array::from(
                documents
                    .iter()
                    .map(|doc| doc.chunk_count)
                    .collect::<Vec<_>>(),
            )),
        ];
        Ok(RecordBatch::try_new(Self::schema(), arrays)?)
    }

    fn build_filter(query: &MessageSearchQuery) -> Option<String> {
        let mut predicates = Vec::new();
        append_string_filter(&mut predicates, "team_id", query.team_id.as_deref());
        append_string_filter(&mut predicates, "run_id", query.run_id.as_deref());
        append_string_filter(
            &mut predicates,
            "conversation_id",
            query.conversation_id.as_deref(),
        );
        append_string_filter(&mut predicates, "task_id", query.task_id.as_deref());
        append_string_filter(&mut predicates, "agent_id", query.agent_id.as_deref());
        append_string_filter(&mut predicates, "session_id", query.session_id.as_deref());
        if let Some(source_kind) = query.source_kind {
            append_string_filter(&mut predicates, "source_kind", Some(source_kind.as_str()));
        }

        (!predicates.is_empty()).then(|| predicates.join(" AND "))
    }
}

#[async_trait]
impl MessageArchiveStore for LanceDbMessageArchive {
    async fn ensure_ready(&self) -> Result<()> {
        let table = self.ensure_table().await?;
        table
            .create_index(&["body_text"], Index::FTS(Default::default()))
            .replace(true)
            .execute()
            .await?;
        Ok(())
    }

    async fn append_documents(&self, documents: &[MessageDocument]) -> Result<()> {
        if documents.is_empty() {
            return Ok(());
        }
        let table = self.ensure_table().await?;
        let batch = Self::documents_to_record_batch(documents)?;
        table.add(batch).execute().await?;
        Ok(())
    }

    async fn search(&self, query: &MessageSearchQuery) -> Result<Vec<MessageSearchHit>> {
        let query_text = query.query_text.trim();
        if query_text.is_empty() || query.limit == 0 {
            return Ok(Vec::new());
        }
        let table = self.ensure_table().await?;

        let search = table
            .query()
            .full_text_search(FullTextSearchQuery::new(query_text.to_string()))
            .select(Select::columns(&[
                "document_id",
                "source_kind",
                "body_text",
                "_score",
            ]))
            .limit(query.limit);
        let search = if let Some(filter) = Self::build_filter(query) {
            search.only_if(filter)
        } else {
            search
        };

        let mut stream = search.execute().await?;

        let mut hits = Vec::new();
        while let Some(batch) = stream.try_next().await? {
            let document_ids = batch
                .column_by_name("document_id")
                .expect("document_id column")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("document_id string array");
            let source_kinds = batch
                .column_by_name("source_kind")
                .expect("source_kind column")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("source_kind string array");
            let body_texts = batch
                .column_by_name("body_text")
                .expect("body_text column")
                .as_any()
                .downcast_ref::<StringArray>()
                .expect("body_text string array");
            let scores = batch
                .column_by_name("_score")
                .and_then(|column| column.as_any().downcast_ref::<Float32Array>());

            for row in 0..batch.num_rows() {
                hits.push(MessageSearchHit {
                    document_id: document_ids.value(row).to_string(),
                    source_kind: parse_source_kind(source_kinds.value(row)),
                    body_text: body_texts.value(row).to_string(),
                    score: scores
                        .and_then(|values| values.is_valid(row).then(|| values.value(row))),
                });
            }
        }
        Ok(hits)
    }
}

fn append_string_filter(predicates: &mut Vec<String>, column: &str, value: Option<&str>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    predicates.push(format!("{column} = '{}'", escape_sql_literal(value)));
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn parse_source_kind(raw: &str) -> MessageDocumentKind {
    match raw {
        "agent_event" => MessageDocumentKind::AgentEvent,
        "team_conversation_message" => MessageDocumentKind::TeamConversationMessage,
        "team_run_event" => MessageDocumentKind::TeamRunEvent,
        "team_actor_message" => MessageDocumentKind::TeamActorMessage,
        "aggregated_acp_message" => MessageDocumentKind::AggregatedAcpMessage,
        _ => MessageDocumentKind::AgentEvent,
    }
}

#[cfg(test)]
mod tests {
    use super::LanceDbMessageArchive;
    use crate::model::{
        MessageArchiveBackend, MessageArchiveConfig, MessageArchiveStore, MessageDocument,
        MessageDocumentKind, MessageSearchQuery,
    };

    #[tokio::test]
    async fn lancedb_archive_can_append_and_search_messages() {
        let config = MessageArchiveConfig {
            backend: MessageArchiveBackend::LanceDb,
            uri: "memory://agenthub-message-archive".to_string(),
            message_table: "messages".to_string(),
        };
        let archive = LanceDbMessageArchive::connect(config)
            .await
            .expect("connect archive");
        archive.ensure_ready().await.expect("ensure ready");
        archive
            .append_documents(&[MessageDocument {
                document_id: "team_conversation_message:conv-1:1".to_string(),
                source_kind: MessageDocumentKind::TeamConversationMessage,
                source_id: "1".to_string(),
                logical_message_id: Some("msg-1".to_string()),
                team_id: Some("team-1".to_string()),
                run_id: None,
                conversation_id: Some("conv-1".to_string()),
                task_id: Some("task-1".to_string()),
                agent_id: None,
                session_id: None,
                body_text: "hello lancedb archive".to_string(),
                payload_json: Some(r#"{"text":"hello lancedb archive"}"#.to_string()),
                created_at: 1,
                event_id_from: None,
                event_id_to: None,
                chunk_count: None,
            }])
            .await
            .expect("append docs");

        let hits = archive
            .search(&MessageSearchQuery {
                query_text: "lancedb".to_string(),
                limit: 5,
                ..Default::default()
            })
            .await
            .expect("search docs");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document_id, "team_conversation_message:conv-1:1");
        assert_eq!(hits[0].body_text, "hello lancedb archive");
        assert!(hits[0].score.is_some());
    }

    #[tokio::test]
    async fn lancedb_archive_applies_scope_filters_and_zero_limit() {
        let config = MessageArchiveConfig {
            backend: MessageArchiveBackend::LanceDb,
            uri: "memory://agenthub-message-archive-filtered".to_string(),
            message_table: "messages".to_string(),
        };
        let archive = LanceDbMessageArchive::connect(config)
            .await
            .expect("connect archive");
        archive.ensure_ready().await.expect("ensure ready");
        archive
            .append_documents(&[
                MessageDocument {
                    document_id: "doc-1".to_string(),
                    source_kind: MessageDocumentKind::TeamConversationMessage,
                    source_id: "1".to_string(),
                    logical_message_id: Some("msg-1".to_string()),
                    team_id: Some("team-a".to_string()),
                    run_id: None,
                    conversation_id: Some("conv-1".to_string()),
                    task_id: None,
                    agent_id: None,
                    session_id: None,
                    body_text: "hello alpha".to_string(),
                    payload_json: None,
                    created_at: 1,
                    event_id_from: None,
                    event_id_to: None,
                    chunk_count: None,
                },
                MessageDocument {
                    document_id: "doc-2".to_string(),
                    source_kind: MessageDocumentKind::TeamConversationMessage,
                    source_id: "2".to_string(),
                    logical_message_id: Some("msg-2".to_string()),
                    team_id: Some("team-b".to_string()),
                    run_id: None,
                    conversation_id: Some("conv-2".to_string()),
                    task_id: None,
                    agent_id: None,
                    session_id: None,
                    body_text: "hello beta".to_string(),
                    payload_json: None,
                    created_at: 2,
                    event_id_from: None,
                    event_id_to: None,
                    chunk_count: None,
                },
            ])
            .await
            .expect("append docs");

        let empty = archive
            .search(&MessageSearchQuery {
                query_text: "hello".to_string(),
                limit: 0,
                ..Default::default()
            })
            .await
            .expect("search docs");
        assert!(empty.is_empty());

        let filtered = archive
            .search(&MessageSearchQuery {
                query_text: "hello".to_string(),
                limit: 5,
                team_id: Some("team-b".to_string()),
                ..Default::default()
            })
            .await
            .expect("search docs");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].document_id, "doc-2");
    }
}
