use super::*;

pub(super) fn directory() -> Option<std::path::PathBuf> {
    std::env::var_os("LOOP_NATIVE_BROWSER_DIR").map(std::path::PathBuf::from)
}

pub(super) struct Browser {
    server: crate::agent::manager::test_browser::BrowserServer,
}

impl Browser {
    pub(super) async fn start(
        fixture: &Fixture,
        local: &str,
        target: &agenthub_rara::RecoveryTarget,
    ) -> Option<Self> {
        let directory = directory()?;
        let server = crate::agent::manager::test_browser::BrowserServer::start(
            &fixture.state,
            directory,
            json!({"team_id":fixture.team_id,"local_session_id":local,"target":target}),
        )
        .await;
        Some(Self { server })
    }

    pub(super) async fn wait_for_review(&self) {
        self.server.wait_for("reviewed").await;
    }

    pub(super) async fn finish(self) {
        self.server.finish().await;
    }
}
