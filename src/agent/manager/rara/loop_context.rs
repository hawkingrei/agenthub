use std::collections::BTreeSet;

use agenthub_agent_domain::loop_runtime::{LoopReservation, LoopTaskContext};
use agenthub_db::loop_runtime::LoopStore;
use serde::Serialize;
use serde_json::Value;

use crate::team::{TeamManager, TeamMemberCardRecord};

#[derive(Clone, Serialize)]
pub(in crate::agent::manager) struct NativeLoopContext {
    pub name: String,
    pub card: TeamMemberCardRecord,
    pub tasks: Vec<LoopTaskContext>,
}

impl NativeLoopContext {
    pub async fn resolve(
        store: &LoopStore,
        reservation: &LoopReservation,
        agent: &crate::agent::AgentRecord,
        spec: &Value,
    ) -> anyhow::Result<Self> {
        let card = TeamManager::member_card_for_launch(spec, agent)?;
        let mut task_ids = BTreeSet::new();
        let mut after = None;
        loop {
            let page = store
                .work_context(
                    reservation,
                    after.as_deref(),
                    256,
                    chrono::Utc::now().timestamp(),
                )
                .await?;
            for source in page.sources.into_iter().filter(|source| !source.revoked) {
                if let Some(task) = source.input.references.task_id {
                    task_ids.insert(task);
                }
            }
            after = page.next_cursor;
            if after.is_none() {
                break;
            }
        }
        // One of the native prompt-source slots belongs to the role and outer identity.
        anyhow::ensure!(
            task_ids.len() < 32,
            "native loop task source capacity exceeded"
        );
        let mut tasks = Vec::with_capacity(task_ids.len());
        for task in task_ids {
            tasks.push(
                store
                    .pin_task_context(reservation, &task, chrono::Utc::now().timestamp())
                    .await?,
            );
        }
        Ok(Self {
            name: agent.name.clone(),
            card,
            tasks,
        })
    }
}
