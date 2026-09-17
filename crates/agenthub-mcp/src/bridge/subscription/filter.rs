use super::*;

#[derive(Default)]
pub(super) struct Filter {
    lists: [bool; 3],
    pub task_ids: Vec<String>,
    resources: Vec<String>,
}

const LIST_FIELDS: [&str; 3] = [
    "toolsListChanged",
    "promptsListChanged",
    "resourcesListChanged",
];

impl Filter {
    pub fn parse(value: &Value) -> Result<Self, McpTransportError> {
        let object = value.as_object().ok_or(McpTransportError::InvalidMessage)?;
        let mut filter = Self::default();
        for (key, value) in object {
            if let Some(index) = LIST_FIELDS.iter().position(|name| *name == key) {
                filter.lists[index] = value.as_bool().ok_or(McpTransportError::InvalidMessage)?;
            } else if matches!(key.as_str(), "resourceSubscriptions" | "taskIds") {
                let items = value
                    .as_array()
                    .filter(|items| items.len() <= 64)
                    .ok_or(McpTransportError::InvalidMessage)?;
                let mut names = Vec::new();
                for item in items {
                    let item = item
                        .as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 4096)
                        .ok_or(McpTransportError::InvalidMessage)?;
                    if names.iter().any(|name| name == item) {
                        return Err(McpTransportError::InvalidMessage);
                    }
                    names.push(item.to_owned());
                }
                if key == "taskIds" {
                    filter.task_ids = names;
                } else {
                    filter.resources = names;
                }
            } else {
                // Unknown filters cannot grant notification or task authority implicitly.
                return Err(McpTransportError::InvalidMessage);
            }
        }
        Ok(filter)
    }

    fn subset_of(&self, requested: &Self) -> bool {
        self.lists
            .iter()
            .zip(requested.lists)
            .all(|(enabled, requested)| !enabled || requested)
            && self
                .task_ids
                .iter()
                .all(|id| requested.task_ids.contains(id))
            && self
                .resources
                .iter()
                .all(|uri| requested.resources.contains(uri))
    }
}

pub(super) enum Event {
    Notification,
    Task,
    Complete,
}

pub(super) struct SubscriptionState {
    pub request_id: Value,
    pub requested: Filter,
    acknowledged: Option<Filter>,
}

impl SubscriptionState {
    pub fn new(request_id: Value, requested: Filter) -> Self {
        Self {
            request_id,
            requested,
            acknowledged: None,
        }
    }

    pub fn observe(
        &mut self,
        message: &Value,
        http_status: u16,
    ) -> Result<Event, McpTransportError> {
        if message_kind(message)? == MessageKind::Response {
            if message.get("error").is_some() {
                if message.get("id") == Some(&self.request_id)
                    || http_status >= 400 && message.get("id").is_none_or(Value::is_null)
                {
                    return Ok(Event::Complete);
                }
            } else if self.acknowledged.is_some()
                && message.get("id") == Some(&self.request_id)
                && message["result"]["resultType"] == "complete"
                && message.pointer("/result/_meta/io.modelcontextprotocol~1subscriptionId")
                    == Some(&self.request_id)
            {
                return Ok(Event::Complete);
            }
            return Err(McpTransportError::InvalidResponse);
        }
        if message_kind(message)? != MessageKind::Notification
            || message.pointer("/params/_meta/io.modelcontextprotocol~1subscriptionId")
                != Some(&self.request_id)
        {
            return Err(McpTransportError::InvalidResponse);
        }
        if message["method"] == "notifications/subscriptions/acknowledged" {
            let filter = Filter::parse(&message["params"]["notifications"])?;
            if self.acknowledged.is_some() || !filter.subset_of(&self.requested) {
                return Err(McpTransportError::InvalidResponse);
            }
            self.acknowledged = Some(filter);
            return Ok(Event::Notification);
        }
        let acknowledged = self
            .acknowledged
            .as_ref()
            .ok_or(McpTransportError::InvalidResponse)?;
        let allowed = match message["method"].as_str() {
            Some("notifications/tools/list_changed") => acknowledged.lists[0],
            Some("notifications/prompts/list_changed") => acknowledged.lists[1],
            Some("notifications/resources/list_changed") => acknowledged.lists[2],
            Some("notifications/resources/updated") => message["params"]["uri"]
                .as_str()
                .is_some_and(|uri| acknowledged.resources.iter().any(|item| item == uri)),
            Some("notifications/tasks") => {
                if message["params"]["taskId"]
                    .as_str()
                    .is_some_and(|id| acknowledged.task_ids.iter().any(|item| item == id))
                {
                    return Ok(Event::Task);
                }
                false
            }
            _ => false,
        };
        if !allowed {
            return Err(McpTransportError::InvalidResponse);
        }
        Ok(Event::Notification)
    }
}
