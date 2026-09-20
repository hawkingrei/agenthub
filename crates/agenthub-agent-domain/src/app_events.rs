//! Signed notifications identify work; they never supply commands or canonical task authority.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{app_tools::valid_name, loop_runtime::validate_loop_id};

pub const APP_EVENT_MAX_BYTES: usize = 8192;
pub const APP_EVENT_MAX_CLASSES: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppEventDeclaration {
    pub name: String,
    pub required_scopes: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppEventNotification {
    pub schema_version: u32,
    pub event_id: String,
    pub cursor: i64,
    pub team_id: String,
    pub actor_id: String,
    pub event_class: String,
}

impl AppEventNotification {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.schema_version == 1, "unsupported app event version");
        anyhow::ensure!(
            valid_name(&self.event_id) && valid_name(&self.event_class) && self.cursor > 0,
            "invalid app event identity"
        );
        validate_loop_id(&self.team_id)?;
        validate_loop_id(&self.actor_id)
    }
}

/// Immutable safe source metadata; App identity is the enclosing source's app_id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppEventAttribution {
    pub event_id: String,
    pub event_class: String,
    pub cursor: i64,
    pub version: i64,
}

impl AppEventAttribution {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            valid_name(&self.event_id)
                && valid_name(&self.event_class)
                && self.cursor > 0
                && self.version > 0,
            "invalid app event attribution"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_runtime::LoopSourceReferences;
    use serde_json::json;

    #[test]
    fn notifications_reject_commands_unbounded_identity_and_unknown_versions() {
        let valid = json!({"schema_version":1,"event_id":"event-1","cursor":1,"team_id":"team","actor_id":"worker","event_class":"document.changed"});
        serde_json::from_value::<AppEventNotification>(valid.clone())
            .unwrap()
            .validate()
            .unwrap();
        for (field, value) in [
            ("schema_version", json!(2)),
            ("cursor", json!(0)),
            ("event_id", json!("x".repeat(129))),
            ("event_class", json!("do this now")),
            ("team_id", json!("")),
            ("actor_id", json!("bad actor")),
        ] {
            let mut invalid = valid.clone();
            invalid[field] = value;
            assert!(
                serde_json::from_value::<AppEventNotification>(invalid)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        for field in ["command", "payload", "task_id", "activation_id"] {
            let mut invalid = valid.clone();
            invalid[field] = json!("injected");
            assert!(serde_json::from_value::<AppEventNotification>(invalid).is_err());
        }
    }

    #[test]
    fn event_attribution_is_optional_for_old_sources_and_requires_app_identity() {
        let old = serde_json::to_value(LoopSourceReferences::default()).unwrap();
        assert!(old.get("app_event").is_none());
        let mut refs: LoopSourceReferences = serde_json::from_value(old).unwrap();
        refs.app_event = Some(AppEventAttribution {
            event_id: "event-1".into(),
            event_class: "changed".into(),
            cursor: 1,
            version: 1,
        });
        assert!(refs.validate().is_err());
        refs.app_id = Some("app-1".into());
        refs.validate().unwrap();
        refs.app_event.as_mut().unwrap().version = 0;
        assert!(refs.validate().is_err());
    }
}
