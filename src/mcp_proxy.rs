use std::{collections::HashMap, sync::Arc};

use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_db::{
    app_registry::{AppActivationPin, AppRegistry},
    mcp_operations::McpOperationStore,
};
use agenthub_mcp::{
    bridge::{McpProxyBinding, McpProxySession},
    budget::McpProxyBudget,
    journal::JournaledMcpClient,
};
use tokio::sync::Mutex;
use tonic::Status;

pub(crate) mod apps;
pub(crate) mod configured;
pub(crate) mod context;

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

#[derive(Clone)]
struct Mount {
    binding: Arc<McpProxyBinding>,
    app: Option<AppActivationPin>,
}

#[derive(Clone)]
struct Session {
    scope: Scope,
    proxy: Arc<McpProxySession>,
    app: Option<AppActivationPin>,
}

type Mounts = HashMap<(Scope, String), Mount>;
type Sessions = HashMap<String, Session>;

pub(crate) struct McpProxyHub {
    pub journal: JournaledMcpClient,
    pub budget: Arc<McpProxyBudget>,
    apps: AppRegistry,
    mounts: Mutex<Mounts>,
    sessions: Mutex<Sessions>,
}

impl McpProxyHub {
    /// Mounts are resolved by trusted launch configuration, never by a provider RPC payload.
    pub(crate) fn new(
        journal: McpOperationStore,
        apps: AppRegistry,
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
                index.insert(key, Mount { binding, app: None }).is_none(),
                "duplicate MCP binding"
            );
        }
        let budget = Arc::new(McpProxyBudget::default());
        Ok(Self {
            journal: JournaledMcpClient::new(journal, budget.delivery.clone()),
            budget,
            apps,
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
        let mount = self
            .mounts
            .lock()
            .await
            .get(&(scope.clone(), server_id.into()))
            .cloned()
            .ok_or_else(|| {
                Status::permission_denied("MCP binding is not available to this activation")
            })?;
        self.authorize_app(executor, mount.app.as_ref()).await?;
        let mut sessions = self.sessions.lock().await;
        let mut retired = Vec::new();
        sessions.retain(|_, session| {
            if session.proxy.is_active() {
                true
            } else {
                retired.push(session.proxy.clone());
                false
            }
        });
        drop(sessions);
        for session in retired {
            let _ = session.shutdown().await;
        }
        let observer =
            self.journal.task_observer(executor).await.map_err(|_| {
                Status::permission_denied("MCP observation owner is no longer active")
            })?;
        let mut sessions = self.sessions.lock().await;
        if sessions.len() >= 128
            || sessions
                .values()
                .filter(|session| session.scope == scope)
                .count()
                >= 32
        {
            return Err(Status::resource_exhausted("MCP session limit reached"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let session = McpProxySession::with_task_observer(
            id.clone(),
            mount.binding,
            self.budget.clone(),
            Some(observer),
        );
        if !session.is_active() {
            return Err(Status::permission_denied("MCP binding has been revoked"));
        }
        sessions.insert(
            id.clone(),
            Session {
                scope,
                proxy: session,
                app: mount.app,
            },
        );
        Ok(id)
    }

    /// Called while the launch operation guard is held, after the immutable snapshot is stored.
    pub(crate) async fn mount(
        &self,
        executor: &LoopReservation,
        binding: Arc<McpProxyBinding>,
    ) -> anyhow::Result<()> {
        self.mount_binding(executor, Mount { binding, app: None })
            .await
    }

    pub(crate) async fn mount_app(
        &self,
        executor: &LoopReservation,
        app: apps::AppLaunchBinding,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            executor.activation_id.as_deref() == Some(&app.pin.activation_id)
                && executor.team_id == app.pin.team_id
                && executor.actor_id == app.pin.actor_id
                && executor.generation >= app.pin.pinned_generation,
            "app mount does not match its activation"
        );
        self.mount_binding(
            executor,
            Mount {
                binding: app.binding,
                app: Some(app.pin),
            },
        )
        .await
    }

    async fn mount_binding(&self, executor: &LoopReservation, mount: Mount) -> anyhow::Result<()> {
        let key = (
            Scope::from_executor(executor)?,
            mount.binding.server_id().to_owned(),
        );
        let mut mounts = self.mounts.lock().await;
        anyhow::ensure!(
            mounts.len() < 1024 && !mounts.contains_key(&key),
            "MCP binding cannot be mounted"
        );
        mounts.insert(key, mount);
        Ok(())
    }

    pub(crate) async fn session(
        &self,
        executor: &LoopReservation,
        id: &str,
    ) -> Result<Arc<McpProxySession>, Status> {
        let session = self.scoped_session(executor, id).await?;
        if let Err(error) = self.authorize_app(executor, session.app.as_ref()).await {
            session.proxy.close();
            return Err(error);
        }
        Ok(session.proxy)
    }

    async fn authorize_app(
        &self,
        executor: &LoopReservation,
        pin: Option<&AppActivationPin>,
    ) -> Result<(), Status> {
        if let Some(pin) = pin {
            let current = self
                .apps
                .authorize_pin(executor, &pin.app_id, true, chrono::Utc::now().timestamp())
                .await
                .map_err(|_| Status::permission_denied("App authorization is no longer active"))?;
            if &current != pin {
                return Err(Status::permission_denied("App activation pin changed"));
            }
        }
        Ok(())
    }

    async fn scoped_session(
        &self,
        executor: &LoopReservation,
        id: &str,
    ) -> Result<Session, Status> {
        if id.len() > 128 {
            return Err(Status::invalid_argument("invalid MCP session reference"));
        }
        let scope = Scope::from_executor(executor)?;
        self.sessions
            .lock()
            .await
            .get(id)
            .filter(|session| session.scope == scope)
            .cloned()
            .ok_or_else(|| {
                Status::permission_denied("MCP session is not available to this activation")
            })
    }

    pub(crate) async fn close(&self, executor: &LoopReservation, id: &str) -> Result<(), Status> {
        // Revocation denies new work, but must not prevent scoped transport cleanup.
        let session = self.scoped_session(executor, id).await?;
        let result = session.proxy.shutdown().await;
        self.sessions.lock().await.remove(id);
        result.map_err(|_| Status::unavailable("MCP upstream session termination failed"))
    }

    pub(crate) async fn release_activation(&self, executor: &LoopReservation) {
        let Ok(scope) = Scope::from_executor(executor) else {
            return;
        };
        self.mounts
            .lock()
            .await
            .retain(|(owner, _), _| owner != &scope);
        let mut removed = Vec::new();
        self.sessions.lock().await.retain(|_, session| {
            if session.scope == scope {
                session.proxy.close();
                removed.push(session.proxy.clone());
                false
            } else {
                true
            }
        });
        for session in removed {
            let _ = session.shutdown().await;
        }
    }
}
