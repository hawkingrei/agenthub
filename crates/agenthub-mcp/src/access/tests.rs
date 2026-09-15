use super::*;
use serde_json::json;

fn names(values: &[&str]) -> McpSelection {
    McpSelection::Names(values.iter().map(|name| (*name).to_owned()).collect())
}

fn policy() -> McpAccessPolicy {
    McpAccessPolicy {
        tools: names(&["write"]),
        resources: names(&["mem://a/item"]),
        resource_templates: names(&["mem://a/{id}"]),
        prompts: names(&["brief"]),
        callbacks: names(&["roots/list"]),
        logging: false,
    }
}

#[test]
fn access_checks_each_named_surface_without_treating_templates_as_prefixes() {
    let policy = policy();
    for (method, allowed, denied) in [
        (
            "tools/call",
            json!({"name":"write"}),
            json!({"name":"admin"}),
        ),
        (
            "resources/read",
            json!({"uri":"mem://a/item"}),
            json!({"uri":"mem://a/other"}),
        ),
        (
            "resources/subscribe",
            json!({"uri":"mem://a/item"}),
            json!({"uri":"mem://b/item"}),
        ),
        (
            "resources/unsubscribe",
            json!({"uri":"mem://a/item"}),
            json!({"uri":"mem://a/{id}"}),
        ),
        (
            "prompts/get",
            json!({"name":"brief"}),
            json!({"name":"secret"}),
        ),
        (
            "completion/complete",
            json!({"ref":{"type":"ref/prompt","name":"brief"}}),
            json!({"ref":{"type":"ref/prompt","name":"secret"}}),
        ),
        (
            "completion/complete",
            json!({"ref":{"type":"ref/resource","uri":"mem://a/{id}"}}),
            json!({"ref":{"type":"ref/resource","uri":"mem://a/item"}}),
        ),
    ] {
        let request = |params| json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
        assert!(
            policy.authorize_request(&request(allowed)).is_ok(),
            "{method}"
        );
        assert_eq!(
            policy.authorize_request(&request(denied)),
            Err(McpPolicyError::Scope),
            "{method}"
        );
        assert_eq!(
            policy.authorize_request(&request(json!({}))),
            Err(McpPolicyError::Scope),
            "{method}"
        );
    }
    for method in [
        "tools/list",
        "resources/list",
        "resources/templates/list",
        "prompts/list",
        "tasks/get",
        "logging/setLevel",
    ] {
        assert_eq!(
            McpAccessPolicy::default()
                .authorize_request(&json!({"jsonrpc":"2.0","id":1,"method":method})),
            Err(McpPolicyError::Scope)
        );
    }
}

#[test]
fn access_checks_every_subscription_filter_and_callback_before_granting_delivery() {
    let policy = policy();
    for filter in [
        json!({"resourceSubscriptions":["mem://a/item"]}),
        json!({"toolsListChanged":true,"promptsListChanged":true}),
    ] {
        assert!(policy.authorize_request(&json!({"jsonrpc":"2.0","id":1,"method":"subscriptions/listen","params":{"notifications":filter}})).is_ok());
    }
    for filter in [
        json!({"resourceSubscriptions":["mem://a/item","mem://b/item"]}),
        json!({"unknown":true}),
    ] {
        assert!(policy.authorize_request(&json!({"jsonrpc":"2.0","id":1,"method":"subscriptions/listen","params":{"notifications":filter}})).is_err());
    }
    assert!(
        policy
            .authorize_server_message(&json!({"jsonrpc":"2.0","id":"roots","method":"roots/list"}))
            .is_ok()
    );
    assert!(
        policy
            .authorize_server_message(
                &json!({"jsonrpc":"2.0","id":"sampling","method":"sampling/createMessage"})
            )
            .is_err()
    );
    assert!(policy.authorize_server_message(&json!({"jsonrpc":"2.0","method":"notifications/resources/updated","params":{"uri":"mem://b/item"}})).is_err());
    for filter in [
        json!({"promptsListChanged":true}),
        json!({"resourcesListChanged":true}),
        json!({"resourceSubscriptions":["mem://a/item"]}),
    ] {
        assert!(McpAccessPolicy::tools_only().authorize_request(&json!({"jsonrpc":"2.0","id":1,"method":"subscriptions/listen","params":{"notifications":filter}})).is_err());
    }
}

#[test]
fn discovery_projection_preserves_allowed_items_extensions_and_error_payloads() {
    let policy = policy();
    for (method, key, allowed, denied) in [
        (
            "resources/list",
            "resources",
            json!({"uri":"mem://a/item","name":"Visible","extension":{"kept":true}}),
            json!({"uri":"mem://b/item","name":"Private"}),
        ),
        (
            "resources/templates/list",
            "resourceTemplates",
            json!({"uriTemplate":"mem://a/{id}","name":"Visible","extension":[1,2]}),
            json!({"uriTemplate":"mem://b/{id}","name":"Private"}),
        ),
        (
            "prompts/list",
            "prompts",
            json!({"name":"brief","arguments":[{"name":"topic"}],"extension":7}),
            json!({"name":"secret"}),
        ),
    ] {
        let mut response = json!({"jsonrpc":"2.0","id":"list","result":{"nextCursor":"opaque-cursor","extension":{"kept":true},key:[allowed,denied]}});
        policy.project_response(method, &mut response).unwrap();
        assert_eq!(response["result"][key], json!([allowed]));
        assert_eq!(response["result"]["nextCursor"], "opaque-cursor");
        assert_eq!(response["result"]["extension"]["kept"], true);
        let mut error = json!({"jsonrpc":"2.0","id":"list","error":{"code":-32000,"message":"upstream","data":{"preserved":true}}});
        let original = error.clone();
        policy.project_response(method, &mut error).unwrap();
        assert_eq!(error, original);
    }
    let mut mixed = json!({"jsonrpc":"2.0","id":1,"result":{"contents":[{"uri":"mem://a/item","text":"visible"},{"uri":"mem://b/item","text":"private"}]}});
    assert!(
        policy
            .project_response("resources/read", &mut mixed)
            .is_err()
    );
    mixed["result"]["resultType"] = "input_required".into();
    assert!(
        policy
            .project_response("resources/read", &mut mixed)
            .is_err()
    );
    let mut deferred = json!({"jsonrpc":"2.0","id":2,"result":{"resultType":"input_required","requestState":{"opaque":1},"inputRequests":[]}});
    let original = deferred.clone();
    policy
        .project_response("resources/read", &mut deferred)
        .unwrap();
    assert_eq!(deferred, original);
    let mut capabilities = json!({"jsonrpc":"2.0","id":1,"result":{"capabilities":{"tools":{},"resources":{},"prompts":{},"logging":{},"completions":{},"experimental":{"vendor":true}},"serverInfo":{"name":"fixture","version":"1"}}});
    McpAccessPolicy::tools_only()
        .project_response("initialize", &mut capabilities)
        .unwrap();
    assert_eq!(
        capabilities["result"]["capabilities"],
        json!({"tools":{},"experimental":{"vendor":true}})
    );
}

#[test]
fn deferred_inputs_require_the_same_callback_grants_as_direct_requests() {
    let policy = policy();
    for method in ["roots/list", "sampling/createMessage", "elicitation/create"] {
        let inputs = json!({"opaque-input":{"method":method,"params":{}}});
        for message in [
            json!({"jsonrpc":"2.0","id":1,"result":{"resultType":"input_required","requestState":"opaque","inputRequests":inputs}}),
            json!({"jsonrpc":"2.0","id":2,"result":{"resultType":"task","taskId":"task","status":"input_required","inputRequests":inputs}}),
            json!({"jsonrpc":"2.0","id":3,"result":{"resultType":"complete","taskId":"task","status":"input_required","inputRequests":inputs}}),
            json!({"jsonrpc":"2.0","method":"notifications/tasks","params":{"taskId":"task","status":"input_required","inputRequests":inputs}}),
        ] {
            assert_eq!(
                policy.authorize_server_message(&message).is_ok(),
                method == "roots/list",
                "{method}: {message}"
            );
        }
    }
    for inputs in [json!([]), json!({"unknown":{"method":"vendor/input"}})] {
        assert!(policy.authorize_server_message(&json!({"jsonrpc":"2.0","id":1,"result":{"resultType":"input_required","inputRequests":inputs}})).is_err());
    }
    // Ordinary result data and error details are not protocol input requests.
    for message in [
        json!({"jsonrpc":"2.0","id":1,"result":{"resultType":"complete","inputRequests":{"data":true}}}),
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"upstream","data":{"inputRequests":{"data":true}}}}),
    ] {
        assert!(policy.authorize_server_message(&message).is_ok());
    }
}
