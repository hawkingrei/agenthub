use super::*;
use serde_json::json;

#[test]
fn client_capabilities_gate_direct_and_deferred_inputs_with_the_same_rules() {
    for (capabilities, method, params, permitted) in [
        (json!({}), "roots/list", json!({}), false),
        (json!({"roots":{}}), "roots/list", json!({}), true),
        (json!({"roots":false}), "roots/list", json!({}), false),
        (
            json!({"sampling":{}}),
            "sampling/createMessage",
            json!({}),
            true,
        ),
        (
            json!({"sampling":{}}),
            "sampling/createMessage",
            json!({"tools":[]}),
            false,
        ),
        (
            json!({"sampling":{"tools":{}}}),
            "sampling/createMessage",
            json!({"tools":[],"toolChoice":{"mode":"auto"}}),
            true,
        ),
        (
            json!({"sampling":{}}),
            "sampling/createMessage",
            json!({"toolChoice":{"mode":"auto"}}),
            false,
        ),
        (
            json!({"sampling":{}}),
            "sampling/createMessage",
            json!({"includeContext":"allServers"}),
            true,
        ),
        (json!({}), "elicitation/create", json!({}), false),
        (
            json!({"elicitation":{}}),
            "elicitation/create",
            json!({}),
            true,
        ),
        (
            json!({"elicitation":{}}),
            "elicitation/create",
            json!({"mode":"url"}),
            false,
        ),
        (
            json!({"elicitation":{"form":{}}}),
            "elicitation/create",
            json!({"mode":"form"}),
            true,
        ),
        (
            json!({"elicitation":{"url":{}}}),
            "elicitation/create",
            json!({"mode":"url"}),
            true,
        ),
        (
            json!({"elicitation":{"url":{}}}),
            "elicitation/create",
            json!({}),
            false,
        ),
        (
            json!({"elicitation":{"form":{},"url":{}}}),
            "elicitation/create",
            json!({"mode":"invented"}),
            false,
        ),
    ] {
        let capabilities = ClientCapabilities::from_value(&capabilities);
        let input = json!({"method":method,"params":params});
        for message in [
            json!({"jsonrpc":"2.0","id":"callback","method":method,"params":params}),
            json!({"jsonrpc":"2.0","id":1,"result":{"resultType":"input_required","inputRequests":{"input":input}}}),
            json!({"jsonrpc":"2.0","id":2,"result":{"resultType":"task","taskId":"task","status":"input_required","inputRequests":{"input":input}}}),
            json!({"jsonrpc":"2.0","id":3,"result":{"resultType":"complete","taskId":"task","status":"input_required","inputRequests":{"input":input}}}),
            json!({"jsonrpc":"2.0","method":"notifications/tasks","params":{"taskId":"task","status":"input_required","inputRequests":{"input":input}}}),
        ] {
            assert_eq!(
                capabilities.authorize_message(&message).is_ok(),
                permitted,
                "{message}"
            );
        }
    }
}

#[test]
fn client_capabilities_do_not_inspect_opaque_result_or_error_content() {
    let capabilities = ClientCapabilities::default();
    let opaque = json!({"inputRequests":{"input":{"method":"elicitation/create"}},"resultType":"input_required"});
    for message in [
        json!({"jsonrpc":"2.0","id":1,"result":{"resultType":"complete","content":[opaque]}}),
        json!({"jsonrpc":"2.0","id":2,"error":{"code":-32000,"message":"upstream","data":opaque}}),
        json!({"jsonrpc":"2.0","id":3,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":4,"method":"vendor/extension","params":opaque}),
    ] {
        assert!(capabilities.authorize_message(&message).is_ok());
    }
    assert!(std::mem::size_of::<ClientCapabilities>() <= 8);
}

#[test]
fn modern_logging_requires_this_requests_opt_in_and_minimum_severity() {
    let log = |level: &str| json!({"jsonrpc":"2.0","method":"notifications/message","params":{"level":level,"data":{"native":"kept"}}});
    for (requested, emitted, allowed) in [
        (None, "warning", false),
        (Some("warning"), "debug", false),
        (Some("warning"), "warning", true),
        (Some("warning"), "emergency", true),
        (Some("debug"), "info", true),
        (Some("debug"), "invalid", false),
    ] {
        let mut request =
            json!({"params":{"_meta":{"io.modelcontextprotocol/clientCapabilities":{}}}});
        if let Some(level) = requested {
            request["params"]["_meta"]["io.modelcontextprotocol/logLevel"] = json!(level);
        }
        assert_eq!(
            ClientCapabilities::from_request(&request)
                .authorize_message(&log(emitted))
                .is_ok(),
            allowed
        );
    }
    assert!(
        ClientCapabilities::from_value(&json!({}))
            .authorize_message(&log("debug"))
            .is_ok(),
        "legacy logging does not require modern opt-in metadata"
    );
}
