use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use agenthub_agent_domain::{
    loop_runtime::LoopReservation,
    mcp_operations::{McpTaskObservation, McpTaskObservationBinding, McpTaskReceipt},
};
use agenthub_db::mcp_operations::McpTaskObservationOwner;
use tokio::{sync::watch, time::Instant};

use super::*;
use crate::{
    http::HttpContext,
    protocol::ProtocolVersion,
    task::{TaskContext, notification_observation, task_digest},
};

#[derive(Clone)]
pub struct JournaledTaskObserver(Arc<Observer>);

struct Observer {
    journal: McpOperationStore,
    owner: McpTaskObservationOwner,
    pending_calls: AtomicUsize,
    receipts: watch::Sender<u64>,
}

#[derive(Clone)]
pub(crate) struct BoundTaskObserver {
    observer: JournaledTaskObserver,
    binding: McpTaskObservationBinding,
    context: TaskContext,
}

pub(super) struct PendingTaskReceipt(JournaledTaskObserver);

impl Drop for PendingTaskReceipt {
    fn drop(&mut self) {
        self.0.0.pending_calls.fetch_sub(1, Ordering::AcqRel);
        self.0
            .0
            .receipts
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
    }
}

impl JournaledMcpClient {
    pub async fn task_observer(
        &self,
        executor: &LoopReservation,
    ) -> Result<JournaledTaskObserver, McpCallError> {
        let owner = self
            .journal
            .authorize_task_observation_owner(executor, now())
            .await
            .map_err(journal_error)?;
        Ok(JournaledTaskObserver(Arc::new(Observer {
            journal: self.journal.clone(),
            owner,
            pending_calls: AtomicUsize::new(0),
            receipts: watch::channel(0).0,
        })))
    }
}

impl JournaledTaskObserver {
    pub(crate) fn bind(
        &self,
        binding: McpTaskObservationBinding,
        http: &HttpContext,
    ) -> Result<Option<BoundTaskObserver>, McpTransportError> {
        if http.version != ProtocolVersion::November2025 {
            return Ok(None);
        }
        Ok(Some(BoundTaskObserver {
            observer: self.clone(),
            binding,
            context: TaskContext::new(http)?,
        }))
    }
}

impl BoundTaskObserver {
    pub(super) fn begin_task_receipt(&self) -> PendingTaskReceipt {
        self.observer.0.pending_calls.fetch_add(1, Ordering::AcqRel);
        PendingTaskReceipt(self.observer.clone())
    }

    fn parse(&self, message: &Value) -> Result<McpTaskObservation, McpTransportError> {
        let receipt = McpTaskReceipt {
            task_digest: task_digest(&message["params"]["taskId"])?,
            version: self.context.version,
            session_digest: self.context.session_digest.clone(),
        };
        let observed = notification_observation(&receipt, message)?;
        Ok(McpTaskObservation {
            receipt,
            response_digest: digest("mcp-task-notification-v1", &message["params"])?,
            outcome: observed.outcome,
            inputs: observed.inputs,
        })
    }

    async fn record(&self, fact: &McpTaskObservation) -> Result<bool, McpTransportError> {
        match self
            .observer
            .0
            .journal
            .record_scoped_task_notification(&self.observer.0.owner, &self.binding, fact, now())
            .await
        {
            Ok(()) => Ok(true),
            Err(error)
                if matches!(
                    error.downcast_ref::<McpJournalError>(),
                    Some(McpJournalError::TaskReceiptMissing)
                ) =>
            {
                Ok(false)
            }
            Err(_) => Err(McpTransportError::InvalidResponse),
        }
    }
}

pub(crate) enum TaskEventDisposition {
    Forward,
    Held,
    Rejected,
}

struct PendingEvent {
    message: Budgeted<Value>,
    fact: McpTaskObservation,
    expires: Instant,
}

/// Facts are committed before delivery credits are acquired. Uncorrelated status messages can
/// wait for an in-flight create-task receipt, while unrelated callbacks keep flowing.
pub(crate) struct TaskEventDrain {
    observer: Option<BoundTaskObserver>,
    changes: watch::Receiver<u64>,
    pending: Vec<PendingEvent>,
    budget: ByteBudget,
    timeout: Duration,
    pub lost: bool,
}

impl TaskEventDrain {
    pub fn new(observer: Option<BoundTaskObserver>, timeout: Duration) -> Self {
        let changes = observer
            .as_ref()
            .map(|observer| observer.observer.0.receipts.subscribe())
            .unwrap_or_else(|| watch::channel(0).1);
        Self {
            observer,
            changes,
            pending: Vec::new(),
            budget: ByteBudget::new(crate::MAX_MESSAGE_BYTES),
            timeout,
            lost: false,
        }
    }

    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changes.clone()
    }
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub async fn accept(&mut self, message: &Value) -> TaskEventDisposition {
        let members = message
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(std::slice::from_ref(message));
        if !members.iter().any(|member| {
            matches!(
                member["method"].as_str(),
                Some("notifications/tasks" | "notifications/tasks/status")
            )
        }) {
            return TaskEventDisposition::Forward;
        }
        let result = async {
            if message.is_array() || message["method"] != "notifications/tasks/status" {
                return Err(McpTransportError::InvalidResponse);
            }
            let observer = self
                .observer
                .as_ref()
                .ok_or(McpTransportError::InvalidResponse)?;
            let fact = observer.parse(message)?;
            if observer.record(&fact).await? {
                return Ok(TaskEventDisposition::Forward);
            }
            if observer.observer.0.pending_calls.load(Ordering::Acquire) == 0
                || self.pending.len() >= 64
            {
                return Err(McpTransportError::InvalidResponse);
            }
            let message = self.budget.retain(message.clone(), json_bytes(message)?)?;
            self.pending.push(PendingEvent {
                message,
                fact,
                expires: Instant::now() + self.timeout,
            });
            Ok(TaskEventDisposition::Held)
        }
        .await;
        match result {
            Ok(disposition) => disposition,
            Err(_) => {
                self.lost = true;
                TaskEventDisposition::Rejected
            }
        }
    }

    pub async fn flush(&mut self) -> Vec<Budgeted<Value>> {
        let Some(observer) = &self.observer else {
            return Vec::new();
        };
        let mut ready = Vec::new();
        let mut waiting = Vec::new();
        for event in self.pending.drain(..) {
            match observer.record(&event.fact).await {
                Ok(true) => ready.push(event.message),
                Ok(false)
                    if observer.observer.0.pending_calls.load(Ordering::Acquire) > 0
                        && Instant::now() < event.expires =>
                {
                    waiting.push(event)
                }
                _ => self.lost = true,
            }
        }
        self.pending = waiting;
        ready
    }

    pub async fn settle(&mut self) -> Vec<Budgeted<Value>> {
        let mut ready = Vec::new();
        loop {
            ready.extend(self.flush().await);
            let Some(deadline) = self.pending.iter().map(|event| event.expires).min() else {
                return ready;
            };
            let _ = tokio::time::timeout_at(deadline, self.changes.changed()).await;
        }
    }

    pub async fn finish(
        &mut self,
        client: &JournaledMcpClient,
        events: &mpsc::Sender<Budgeted<HttpEvent>>,
    ) {
        for message in self.settle().await {
            let (message, _) = message.into_parts();
            if !client.deliver(
                HttpEvent {
                    message: Some(message),
                    cursor: None,
                    retry: None,
                },
                events,
            ) {
                self.lost = true;
            }
        }
    }
}
