pub(super) use super::runtime_view_builders::parse_team_member_specs;

#[derive(Debug, Clone)]
pub(super) struct TeamMemberSpecView {
    pub(super) member_id: String,
    pub(super) role: String,
    pub(super) description: Option<String>,
}
