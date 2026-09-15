use std::{collections::HashMap, sync::Arc};

use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_db::mcp_operations::McpOperationStore;
use agenthub_mcp::{
    bridge::{McpProxyBinding, McpProxySession},
    budget::McpProxyBudget,
    journal::JournaledMcpClient,
};
use tokio::sync::Mutex;
use tonic::Status;

pub(crate) mod configured;

pub(crate) const MCP_RPC_MESSAGE_LIMIT: usize = agenthub_mcp::MAX_MESSAGE_BYTES + 65_536;

#[derive(Clone, PartialEq, Eq, Hash)]
struct Scope {
    team: String,
    actor: String,
    activation: String,
    generation: i64,
}

impl Scope {
    fn from_executor(executor: &LoopReservation) -> Result<Self, Status> {
        Ok(Self {
            team: executor.team_id.clone(),
            actor: executor.actor_id.clone(),
            activation: executor
                .activation_id
                .clone()
                .ok_or_else(|| Status::permission_denied("MCP requires an activation"))?,
            generation: executor.generation,
        })
    }
}

type Mounts = HashMap<(Scope, String), Arc<McpProxyBinding>>;
type Sessions = HashMap<String, (Scope, Arc<McpProxySession>)>;

pub(crate) struct McpProxyHub {
    pub journal: JournaledMcpClient,
    pub budget: Arc<McpProxyBudget>,
    mounts: Mutex<Mounts>,
    sessions: Mutex<Sessions>,
}

impl McpProxyHub {
    /// Mounts are resolved by trusted launch configuration, never by a provider RPC payload.
    pub(crate) fn new(
        journal: McpOperationStore,
        mounts: Vec<(LoopReservation, Arc<McpProxyBinding>)>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(mounts.len() <= 1024, "too many MCP bindings");
        let mut index = HashMap::new();
        for (executor, binding) in mounts {
            let key = (
                Scope::from_executor(&executor)?,
                binding.server_id().to_owned(),
            );
            anyhow::ensure!(
                index.insert(key, binding).is_none(),
                "duplicate MCP binding"
            );
        }
        let budget = Arc::new(McpProxyBudget::default());
        Ok(Self {
            journal: JournaledMcpClient::new(journal, budget.delivery.clone()),
            budget,
            mounts: Mutex::new(index),
            sessions: Mutex::new(HashMap::new()),
        })
    }

    pub(crate) async fn open(
        &self,
        executor: &LoopReservation,
        server_id: &str,
    ) -> Result<String, Status> {
        if server_id.is_empty() || server_id.len() > 128 {
            return Err(Status::invalid_argument("invalid MCP server reference"));
        }
        let scope = Scope::from_executor(executor)?;
        let binding = self
            .mounts
            .lock()
            .await
            .get(&(scope.clone(), server_id.into()))
            .cloned()
            .ok_or_else(|| {
                Status::permission_denied("MCP binding is not available to this activation")
            })?;
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|_, (_, session)| session.is_active());
        if sessions.len() >= 128
            || sessions
                .values()
                .filter(|(owner, _)| owner == &scope)
                .count()
                >= 8
        {
            return Err(Status::resource_exhausted("MCP session limit reached"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let session = McpProxySession::new(id.clone(), binding, self.budget.clone());
        if !session.is_active() {
            return Err(Status::permission_denied("MCP binding has been revoked"));
        }
        sessions.insert(id.clone(), (scope, session));
        Ok(id)
    }

    /// Called while the launch operation guard is held, after the immutable snapshot is stored.
    pub(crate) async fn mount(
        &self,
        executor: &LoopReservation,
        binding: Arc<McpProxyBinding>,
    ) -> anyhow::Result<()> {
        let key = (
            Scope::from_executor(executor)?,
            binding.server_id().to_owned(),
        );
        let mut mounts = self.mounts.lock().await;
        anyhow::ensure!(
            mounts.len() < 1024 && !mounts.contains_key(&key),
            "MCP binding cannot be mounted"
        );
        mounts.insert(key, binding);
        Ok(())
    }

    pub(crate) async fn session(
        &self,
        executor: &LoopReservation,
        id: &str,
    ) -> Result<Arc<McpProxySession>, Status> {
        if id.len() > 128 {
            return Err(Status::invalid_argument("invalid MCP session reference"));
        }
        let scope = Scope::from_executor(executor)?;
        self.sessions
            .lock()
            .await
            .get(id)
            .filter(|(owner, _)| owner == &scope)
            .map(|(_, session)| session.clone())
            .ok_or_else(|| {
                Status::permission_denied("MCP session is not available to this activation")
            })
    }

    pub(crate) async fn close(&self, executor: &LoopReservation, id: &str) -> Result<(), Status> {
        let session = self.session(executor, id).await?;
        session.close();
        self.sessions.lock().await.remove(id);
        Ok(())
    }

    pub(crate) async fn release_activation(&self, executor: &LoopReservation) {
        let Ok(scope) = Scope::from_executor(executor) else {
            return;
        };
        self.mounts
            .lock()
            .await
            .retain(|(owner, _), _| owner != &scope);
        self.sessions.lock().await.retain(|_, (owner, session)| {
            if owner == &scope {
                session.close();
                false
            } else {
                true
            }
        });
    }
}
