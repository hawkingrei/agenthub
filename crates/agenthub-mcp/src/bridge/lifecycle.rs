use std::time::Duration;

use super::*;

impl McpProxySession {
    /// The caller holds lifecycle admission through retirement. Only callback replies can be
    /// concurrent, and their permits must settle before the failed upstream session is deleted.
    pub(super) async fn retire_failed_initialization(&self) -> Result<(), McpTransportError> {
        self.listen_ready.store(false, Ordering::Release);
        self.protocol.lock().await.initialization_failed();
        let _callbacks = self
            .callback_slots
            .acquire_many(CALLBACK_SLOTS)
            .await
            .map_err(|_| McpTransportError::Disconnected)?;
        self.callbacks.lock().await.clear();
        let mut discovery = self.discovery.lock().await;
        let generation = discovery.generation.saturating_add(1);
        *discovery = Discovery {
            generation,
            ..Discovery::default()
        };
        drop(discovery);
        let context = self.upstream_context.lock().await.clone();
        if let Some(context) = context.filter(|context| context.session_id.is_some()) {
            self.terminate_upstream(&context).await?;
        }
        // Keep the context on failure so final proxy shutdown can still clean it up.
        *self.upstream_context.lock().await = None;
        Ok(())
    }

    /// Close admission immediately, but let already admitted writes land their factual results
    /// before asking the upstream to destroy its session. DELETE itself has a bounded lifetime.
    pub async fn shutdown(&self) -> Result<(), McpTransportError> {
        self.close();
        let _close = self.close_gate.lock().await;
        let _settled = self.exchanges.write().await;
        if self.close_sent.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let context = self.protocol.lock().await.http_context().or(self
            .upstream_context
            .lock()
            .await
            .clone());
        let Some(context) = context.filter(|context| context.session_id.is_some()) else {
            return Ok(());
        };
        self.terminate_upstream(&context).await
    }

    async fn terminate_upstream(&self, context: &HttpContext) -> Result<(), McpTransportError> {
        let request = self.binding.policy.transport.prepare_close(context)?;
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut response = self.binding.policy.transport.send(request).await?;
            if !matches!(response.status_code(), 200..=299 | 404 | 405) {
                return Err(McpTransportError::HttpStatus(response.status_code()));
            }
            response.next_event().await?;
            Ok(())
        })
        .await
        .map_err(|_| McpTransportError::Deadline)?
    }
}
