mod dispatcher;
mod payload;
mod selection;
pub use dispatcher::{TeamPermissionReviewDispatcher, TeamPermissionReviewDispatcherSettings};
pub(crate) use selection::resolve_team_permission_review_target;
use std::time::Duration;

const TEAM_PERMISSION_REVIEW_PAYLOAD_TYPE: &str = "permission_review_request";
const TEAM_HUMAN_PERMISSION_CARD_PAYLOAD_TYPE: &str = "permission_review_card";
const DEFAULT_TEAM_PERMISSION_REVIEW_HUMAN_FALLBACK_DELAY: Duration = Duration::from_secs(40);

#[cfg(test)]
mod tests_support;

#[cfg(test)]
mod tests;
