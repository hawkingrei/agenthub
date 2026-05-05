use std::sync::Arc;

use anyhow::{Result, anyhow};
use arrow_array::{
    Array, ArrayRef, Float32Array, Int64Array, RecordBatch, RecordBatchIterator, StringArray,
    UInt32Array,
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
            Field::new("authority_message_id", DataType::Int64, true),
            Field::new("correlation_id", DataType::Utf8, true),
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
        let table = match self
            .connection
            .open_table(&self.config.message_table)
            .execute()
            .await
        {
            Ok(table) => table,
            Err(lancedb::Error::TableNotFound { .. }) => {
                self.connection
                    .create_empty_table(&self.config.message_table, Self::schema())
                    .mode(CreateTableMode::exist_ok(|request| request))
                    .execute()
                    .await?
            }
            Err(err) => return Err(err.into()),
        };
        self.ensure_message_identity_columns(&table).await?;
        Ok(table)
    }

    async fn ensure_message_identity_columns(&self, table: &lancedb::Table) -> Result<()> {
        let schema = table.schema().await?;
        let mut transforms = Vec::new();
        if schema.field_with_name("authority_message_id").is_err() {
            transforms.push((
                "authority_message_id".to_string(),
                "cast(NULL as bigint)".to_string(),
            ));
        }
        if schema.field_with_name("correlation_id").is_err() {
            transforms.push((
                "correlation_id".to_string(),
                "cast(NULL as string)".to_string(),
            ));
        }
        if transforms.is_empty() {
            return Ok(());
        }

        table
            .add_columns(
                lancedb::table::NewColumnTransform::SqlExpressions(transforms),
                None,
            )
            .await?;
        Ok(())
    }

    fn documents_to_record_batch(documents: &[MessageDocument]) -> Result<RecordBatch> {
        let arrays: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| Some(doc.document_id.as_str())),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| Some(doc.source_kind.as_str())),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| Some(doc.source_id.as_str())),
            )),
            Arc::new(StringArray::from_iter(
                documents
                    .iter()
                    .map(|doc| doc.logical_message_id.as_deref()),
            )),
            Arc::new(Int64Array::from_iter(
                documents.iter().map(|doc| doc.authority_message_id),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| doc.correlation_id.as_deref()),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| doc.team_id.as_deref()),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| doc.run_id.as_deref()),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| doc.conversation_id.as_deref()),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| doc.task_id.as_deref()),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| doc.agent_id.as_deref()),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| doc.session_id.as_deref()),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| Some(doc.body_text.as_str())),
            )),
            Arc::new(StringArray::from_iter(
                documents.iter().map(|doc| doc.payload_json.as_deref()),
            )),
            Arc::new(Int64Array::from_iter(
                documents.iter().map(|doc| Some(doc.created_at)),
            )),
            Arc::new(Int64Array::from_iter(
                documents.iter().map(|doc| doc.event_id_from),
            )),
            Arc::new(Int64Array::from_iter(
                documents.iter().map(|doc| doc.event_id_to),
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
        append_i64_filter(
            &mut predicates,
            "authority_message_id",
            query.authority_message_id,
        );
        append_string_filter(
            &mut predicates,
            "correlation_id",
            query.correlation_id.as_deref(),
        );
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
        if let Err(err) = table
            .create_index(&["body_text"], Index::FTS(Default::default()))
            .replace(false)
            .execute()
            .await
            && !is_existing_index_error(&err)
        {
            return Err(err.into());
        }
        Ok(())
    }

    async fn append_documents(&self, documents: &[MessageDocument]) -> Result<()> {
        if documents.is_empty() {
            return Ok(());
        }
        let table = self.ensure_table().await?;
        let batch = Self::documents_to_record_batch(documents)?;
        let reader = RecordBatchIterator::new(vec![Ok(batch)], Self::schema());
        let mut merge = table.merge_insert(&["document_id"]);
        merge.when_matched_update_all(None);
        merge.when_not_matched_insert_all();
        merge.execute(Box::new(reader)).await?;
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
                "authority_message_id",
                "correlation_id",
                "team_id",
                "run_id",
                "conversation_id",
                "task_id",
                "agent_id",
                "session_id",
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
            let document_ids = required_string_array(&batch, "document_id")?;
            let source_kinds = required_string_array(&batch, "source_kind")?;
            let body_texts = required_string_array(&batch, "body_text")?;
            let authority_message_ids = optional_i64_array(&batch, "authority_message_id")?;
            let correlation_ids = optional_string_array(&batch, "correlation_id")?;
            let team_ids = optional_string_array(&batch, "team_id")?;
            let run_ids = optional_string_array(&batch, "run_id")?;
            let conversation_ids = optional_string_array(&batch, "conversation_id")?;
            let task_ids = optional_string_array(&batch, "task_id")?;
            let agent_ids = optional_string_array(&batch, "agent_id")?;
            let session_ids = optional_string_array(&batch, "session_id")?;
            let scores = batch
                .column_by_name("_score")
                .and_then(|column| column.as_any().downcast_ref::<Float32Array>());

            for row in 0..batch.num_rows() {
                let Some(source_kind) = parse_source_kind(source_kinds.value(row)) else {
                    continue;
                };
                hits.push(MessageSearchHit {
                    document_id: document_ids.value(row).to_string(),
                    source_kind,
                    body_text: body_texts.value(row).to_string(),
                    score: scores
                        .and_then(|values| values.is_valid(row).then(|| values.value(row))),
                    authority_message_id: authority_message_ids
                        .and_then(|values| values.is_valid(row).then(|| values.value(row))),
                    correlation_id: optional_string_value(correlation_ids, row),
                    team_id: optional_string_value(team_ids, row),
                    run_id: optional_string_value(run_ids, row),
                    conversation_id: optional_string_value(conversation_ids, row),
                    task_id: optional_string_value(task_ids, row),
                    agent_id: optional_string_value(agent_ids, row),
                    session_id: optional_string_value(session_ids, row),
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

fn append_i64_filter(predicates: &mut Vec<String>, column: &str, value: Option<i64>) {
    let Some(value) = value else {
        return;
    };
    predicates.push(format!("{column} = {value}"));
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn required_string_array<'a>(batch: &'a RecordBatch, column: &str) -> Result<&'a StringArray> {
    let array = batch
        .column_by_name(column)
        .ok_or_else(|| anyhow!("missing search result column: {column}"))?;
    array
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| anyhow!("search result column {column} is not Utf8"))
}

fn optional_string_array<'a>(
    batch: &'a RecordBatch,
    column: &str,
) -> Result<Option<&'a StringArray>> {
    let Some(array) = batch.column_by_name(column) else {
        return Ok(None);
    };
    array
        .as_any()
        .downcast_ref::<StringArray>()
        .map(Some)
        .ok_or_else(|| anyhow!("search result column {column} is not Utf8"))
}

fn optional_i64_array<'a>(batch: &'a RecordBatch, column: &str) -> Result<Option<&'a Int64Array>> {
    let Some(array) = batch.column_by_name(column) else {
        return Ok(None);
    };
    array
        .as_any()
        .downcast_ref::<Int64Array>()
        .map(Some)
        .ok_or_else(|| anyhow!("search result column {column} is not Int64"))
}

fn optional_string_value(values: Option<&StringArray>, row: usize) -> Option<String> {
    values.and_then(|values| values.is_valid(row).then(|| values.value(row).to_string()))
}

fn parse_source_kind(raw: &str) -> Option<MessageDocumentKind> {
    match raw {
        "agent_event" => Some(MessageDocumentKind::AgentEvent),
        "team_conversation_message" => Some(MessageDocumentKind::TeamConversationMessage),
        "team_run_event" => Some(MessageDocumentKind::TeamRunEvent),
        "team_actor_message" => Some(MessageDocumentKind::TeamActorMessage),
        "aggregated_acp_message" => Some(MessageDocumentKind::AggregatedAcpMessage),
        _ => None,
    }
}

fn is_existing_index_error(err: &lancedb::Error) -> bool {
    match err {
        lancedb::Error::Other { message, .. } => message.contains("already exists"),
        lancedb::Error::Lance { source } => source.to_string().contains("already exists"),
        lancedb::Error::External { source } => source.to_string().contains("already exists"),
        _ => err.to_string().contains("already exists"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{LanceDbMessageArchive, escape_sql_literal, parse_source_kind};
    use crate::model::{
        MessageArchiveBackend, MessageArchiveConfig, MessageArchiveStore, MessageDocument,
        MessageDocumentKind, MessageSearchQuery,
    };
    use arrow_schema::{DataType, Field, Schema, SchemaRef};

    fn legacy_schema() -> SchemaRef {
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
                authority_message_id: Some(101),
                correlation_id: Some("corr-1".to_string()),
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
        assert_eq!(hits[0].authority_message_id, Some(101));
        assert_eq!(hits[0].correlation_id.as_deref(), Some("corr-1"));
        assert_eq!(hits[0].team_id.as_deref(), Some("team-1"));
        assert_eq!(hits[0].conversation_id.as_deref(), Some("conv-1"));
        assert_eq!(hits[0].task_id.as_deref(), Some("task-1"));
        assert!(hits[0].score.is_some());

        archive
            .ensure_ready()
            .await
            .expect("ensure ready stays idempotent");
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
                    authority_message_id: Some(101),
                    correlation_id: Some("corr-alpha".to_string()),
                    team_id: Some("team-a".to_string()),
                    run_id: Some("run-a".to_string()),
                    conversation_id: Some("conv-1".to_string()),
                    task_id: Some("task-a".to_string()),
                    agent_id: Some("agent-a".to_string()),
                    session_id: Some("session-a".to_string()),
                    body_text: "hello alpha".to_string(),
                    payload_json: None,
                    created_at: 1,
                    event_id_from: None,
                    event_id_to: None,
                    chunk_count: None,
                },
                MessageDocument {
                    document_id: "doc-2".to_string(),
                    source_kind: MessageDocumentKind::TeamRunEvent,
                    source_id: "2".to_string(),
                    logical_message_id: Some("msg-2".to_string()),
                    authority_message_id: None,
                    correlation_id: Some("corr-beta".to_string()),
                    team_id: Some("team-b".to_string()),
                    run_id: Some("run-b".to_string()),
                    conversation_id: Some("conv-2".to_string()),
                    task_id: Some("task-b".to_string()),
                    agent_id: Some("agent-b".to_string()),
                    session_id: Some("session-b".to_string()),
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
                authority_message_id: None,
                correlation_id: Some("corr-beta".to_string()),
                team_id: Some("team-b".to_string()),
                run_id: Some("run-b".to_string()),
                conversation_id: Some("conv-2".to_string()),
                task_id: Some("task-b".to_string()),
                agent_id: Some("agent-b".to_string()),
                session_id: Some("session-b".to_string()),
                source_kind: Some(MessageDocumentKind::TeamRunEvent),
            })
            .await
            .expect("search docs");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].document_id, "doc-2");
        assert!(filtered[0].score.is_some());

        let authority_filtered = archive
            .search(&MessageSearchQuery {
                query_text: "hello".to_string(),
                limit: 5,
                authority_message_id: Some(101),
                correlation_id: Some("corr-alpha".to_string()),
                ..Default::default()
            })
            .await
            .expect("search docs");
        assert_eq!(authority_filtered.len(), 1);
        assert_eq!(authority_filtered[0].document_id, "doc-1");
    }

    #[tokio::test]
    async fn lancedb_archive_upserts_documents_by_document_id() {
        let config = MessageArchiveConfig {
            backend: MessageArchiveBackend::LanceDb,
            uri: "memory://agenthub-message-archive-upsert".to_string(),
            message_table: "messages".to_string(),
        };
        let archive = LanceDbMessageArchive::connect(config)
            .await
            .expect("connect archive");
        archive.ensure_ready().await.expect("ensure ready");

        let mut document = MessageDocument {
            document_id: "team_run_event:run-1:1".to_string(),
            source_kind: MessageDocumentKind::TeamRunEvent,
            source_id: "1".to_string(),
            logical_message_id: None,
            authority_message_id: Some(1),
            correlation_id: None,
            team_id: Some("team-1".to_string()),
            run_id: Some("run-1".to_string()),
            conversation_id: None,
            task_id: None,
            agent_id: None,
            session_id: None,
            body_text: "first archive body".to_string(),
            payload_json: None,
            created_at: 1,
            event_id_from: Some(1),
            event_id_to: Some(1),
            chunk_count: None,
        };
        archive
            .append_documents(&[document.clone()])
            .await
            .expect("append first doc");

        document.body_text = "second archive body".to_string();
        archive
            .append_documents(&[document])
            .await
            .expect("upsert doc");

        let hits = archive
            .search(&MessageSearchQuery {
                query_text: "second".to_string(),
                limit: 5,
                run_id: Some("run-1".to_string()),
                ..Default::default()
            })
            .await
            .expect("search docs");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document_id, "team_run_event:run-1:1");
        assert_eq!(hits[0].body_text, "second archive body");
    }

    #[tokio::test]
    async fn lancedb_archive_migrates_existing_tables_without_identity_columns() {
        let config = MessageArchiveConfig {
            backend: MessageArchiveBackend::LanceDb,
            uri: "memory://agenthub-message-archive-legacy".to_string(),
            message_table: "messages".to_string(),
        };
        let archive = LanceDbMessageArchive::connect(config)
            .await
            .expect("connect archive");
        archive
            .connection
            .create_empty_table(&archive.config.message_table, legacy_schema())
            .execute()
            .await
            .expect("create legacy table");

        archive.ensure_ready().await.expect("migrate legacy table");

        let table = archive.ensure_table().await.expect("open migrated table");
        let schema = table.schema().await.expect("table schema");
        assert!(schema.field_with_name("authority_message_id").is_ok());
        assert!(schema.field_with_name("correlation_id").is_ok());

        archive
            .append_documents(&[MessageDocument {
                document_id: "legacy-doc-1".to_string(),
                source_kind: MessageDocumentKind::TeamConversationMessage,
                source_id: "1".to_string(),
                logical_message_id: Some("msg-legacy-1".to_string()),
                authority_message_id: Some(501),
                correlation_id: Some("corr-legacy".to_string()),
                team_id: Some("team-legacy".to_string()),
                run_id: None,
                conversation_id: Some("conv-legacy".to_string()),
                task_id: Some("task-legacy".to_string()),
                agent_id: None,
                session_id: None,
                body_text: "hello legacy".to_string(),
                payload_json: None,
                created_at: 1,
                event_id_from: None,
                event_id_to: None,
                chunk_count: None,
            }])
            .await
            .expect("append migrated docs");

        let hits = archive
            .search(&MessageSearchQuery {
                query_text: "legacy".to_string(),
                limit: 5,
                authority_message_id: Some(501),
                correlation_id: Some("corr-legacy".to_string()),
                ..Default::default()
            })
            .await
            .expect("search migrated docs");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document_id, "legacy-doc-1");
    }

    #[test]
    fn parse_source_kind_rejects_unknown_values() {
        assert_eq!(
            parse_source_kind("team_actor_message"),
            Some(MessageDocumentKind::TeamActorMessage)
        );
        assert_eq!(parse_source_kind("unknown_kind"), None);
    }

    #[test]
    fn escape_sql_literal_doubles_quotes() {
        assert_eq!(escape_sql_literal("team'o"), "team''o");
    }
}
