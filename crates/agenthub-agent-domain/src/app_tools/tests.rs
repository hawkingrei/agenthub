use super::*;
use serde_json::json;

fn manifest() -> AppManifest {
    AppManifest {
        schema_version: 1,
        scopes: ["records:write".into(), "records:read".into()].into(),
        tools: vec![AppTool {
            name: "create_record".into(),
            input_schema: json!({"type":"object","properties":{"key":{"type":"string","minLength":1},"text":{"type":"string","maxLength":64}},"required":["key","text"],"additionalProperties":false}),
            output_schema: Some(
                json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
            ),
            required_scopes: ["records:write".into()].into(),
            replay: AppReplayPolicy::StableIdentity {
                property_path: vec!["key".into()],
            },
        }],
    }
}

#[test]
fn pinned_schemas_and_scopes_are_checked_at_each_call_without_argument_rewriting() {
    let manifest = manifest();
    let compiled = manifest.compile().unwrap();
    let granted = ["records:write".into()].into();
    assert_eq!(
        compiled.allowed_tools(&granted).unwrap(),
        ["create_record".into()].into()
    );
    assert!(compiled.allowed_tools(&["foreign".into()].into()).is_err());
    assert!(
        compiled
            .allowed_tools(&["records:read".into()].into())
            .unwrap()
            .is_empty()
    );
    let tool = &manifest.tools[0];
    let declaration = json!({"name":tool.name,"inputSchema":tool.input_schema,"outputSchema":tool.output_schema,"annotations":{"idempotentHint":false}});
    compiled
        .validate_declaration(&declaration, &granted)
        .unwrap();
    let arguments = json!({"key":"request-one","text":"selected content"});
    compiled
        .validate_arguments(&tool.name, &tool.input_schema, &arguments, &granted)
        .unwrap();
    for arguments in [
        json!({"key":"","text":"x"}),
        json!({"key":"one","text":42}),
        json!({"key":"one","text":"x","extra":true}),
    ] {
        assert!(
            compiled
                .validate_arguments(&tool.name, &tool.input_schema, &arguments, &granted)
                .is_err()
        );
    }
    assert!(
        compiled
            .validate_arguments("undeclared", &tool.input_schema, &arguments, &granted)
            .is_err()
    );
    assert!(
        compiled
            .validate_arguments(&tool.name, &tool.input_schema, &arguments, &BTreeSet::new())
            .is_err()
    );
    let mut changed = declaration.clone();
    changed["outputSchema"] = json!({"type":"object"});
    assert!(compiled.validate_declaration(&changed, &granted).is_err());
    changed = declaration;
    changed["inputSchema"] = json!({"type":"object"});
    assert!(compiled.validate_declaration(&changed, &granted).is_err());
    compiled
        .validate_output(&tool.name, &json!({"id":"receipt"}))
        .unwrap();
    assert!(
        compiled
            .validate_output(&tool.name, &json!({"id":17}))
            .is_err()
    );
    assert_eq!(
        arguments,
        json!({"key":"request-one","text":"selected content"})
    );
}

#[test]
fn schemas_are_local_bounded_and_fail_with_payload_free_errors() {
    for schema in [
        json!({"type":"object","$ref":"https://example.invalid/private"}),
        json!({"type":"object","$ref":"file:///tmp/private-schema.json"}),
        json!({"type":"object","properties":{"key":{"type":"string","pattern":"(a+)\\1"}}}),
        json!({"type":"object","$schema":"http://json-schema.org/draft-07/schema#"}),
    ] {
        let mut manifest = manifest();
        manifest.tools[0].input_schema = schema;
        manifest.tools[0].replay = AppReplayPolicy::NonIdempotent;
        let error = manifest.compile().err().unwrap().to_string();
        assert!(!error.contains("private"));
        assert!(!error.contains("example.invalid"));
    }
    let mut oversized = manifest();
    oversized.tools[0].input_schema["description"] = json!("x".repeat(APP_MANIFEST_MAX_BYTES));
    assert!(oversized.compile().is_err());
    let mut undeclared = manifest();
    undeclared.tools[0]
        .required_scopes
        .insert("undeclared".into());
    assert!(undeclared.compile().is_err());
}

#[test]
fn versions_names_duplicates_and_stable_identities_are_explicit() {
    let original = manifest();
    let mut candidate = original.clone();
    candidate.schema_version = 2;
    assert!(candidate.compile().is_err());
    candidate = original.clone();
    candidate.tools.push(candidate.tools[0].clone());
    assert!(candidate.compile().is_err());
    candidate = original.clone();
    candidate.tools[0].name = "bad\nname".into();
    assert!(candidate.compile().is_err());
    candidate = original.clone();
    candidate.tools[0].input_schema["required"] = json!(["text"]);
    assert!(candidate.compile().is_err());
    let mut wire = serde_json::to_value(original).unwrap();
    wire["credential"] = json!("untrusted-field");
    assert!(serde_json::from_value::<AppManifest>(wire).is_err());
}
