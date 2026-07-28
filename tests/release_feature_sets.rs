use toml::Value;

const ROOT_MANIFEST: &str = include_str!("../Cargo.toml");
const OBJECT_STORE_MANIFEST: &str = include_str!("../crates/agenthub-object-store/Cargo.toml");
const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release.yml");
const RELEASE_PREBUILD_WORKFLOW: &str = include_str!("../.github/workflows/release-prebuild.yml");

#[test]
fn object_store_s3_stays_explicit_opt_in() {
    let root_manifest = parse_manifest(ROOT_MANIFEST);
    let root_features = feature_table(&root_manifest);
    assert_feature_equals(root_features, "default", &[]);
    assert_feature_equals(
        root_features,
        "object-store-s3",
        &["agenthub-object-store/s3"],
    );

    for release_feature in ["release-vendored-openssl", "release-lance-fp16", "rocksdb"] {
        let members = feature_members(root_features, release_feature);
        assert!(
            !members
                .iter()
                .any(|member| member == "object-store-s3" || member == "agenthub-object-store/s3"),
            "{release_feature} must not enable S3 support implicitly: {members:?}"
        );
    }

    let object_store_manifest = parse_manifest(OBJECT_STORE_MANIFEST);
    let object_store_features = feature_table(&object_store_manifest);
    assert_feature_equals(object_store_features, "default", &[]);
    assert_feature_equals(
        object_store_features,
        "s3",
        &["opendal/http-transport-reqwest", "opendal/services-s3"],
    );
}

#[test]
fn release_workflows_do_not_enable_s3_feature_sets() {
    for (name, workflow) in [
        ("release.yml", RELEASE_WORKFLOW),
        ("release-prebuild.yml", RELEASE_PREBUILD_WORKFLOW),
    ] {
        assert!(
            !workflow.contains("--all-features"),
            "{name} must not use --all-features because that would bypass opt-in release features"
        );
        assert!(
            !workflow.contains("object-store-s3"),
            "{name} must not include the root S3 release feature before a reviewed release decision"
        );
        assert!(
            !workflow.contains("agenthub-object-store/s3"),
            "{name} must not include the object-store crate S3 feature before a reviewed release decision"
        );

        let matrix_features = workflow_matrix_features(workflow);
        assert!(
            !matrix_features.is_empty(),
            "{name} should keep release feature sets explicit in the matrix"
        );
        for features in matrix_features {
            assert!(
                !features
                    .split(',')
                    .any(|feature| feature.trim() == "object-store-s3"
                        || feature.trim() == "agenthub-object-store/s3"),
                "{name} matrix feature set must not include S3 before a reviewed release decision: {features}"
            );
        }
    }
}

fn parse_manifest(source: &str) -> Value {
    toml::from_str(source).expect("manifest should parse as TOML")
}

fn feature_table(manifest: &Value) -> &toml::map::Map<String, Value> {
    manifest
        .get("features")
        .and_then(Value::as_table)
        .expect("manifest should define a [features] table")
}

fn assert_feature_equals(features: &toml::map::Map<String, Value>, name: &str, expected: &[&str]) {
    let members = feature_members(features, name);
    assert_eq!(members, expected, "unexpected feature members for {name}");
}

fn feature_members(features: &toml::map::Map<String, Value>, name: &str) -> Vec<String> {
    features
        .get(name)
        .unwrap_or_else(|| panic!("feature {name} should exist"))
        .as_array()
        .unwrap_or_else(|| panic!("feature {name} should be an array"))
        .iter()
        .map(|member| {
            member
                .as_str()
                .unwrap_or_else(|| panic!("feature {name} members should be strings"))
                .to_owned()
        })
        .collect()
}

fn workflow_matrix_features(workflow: &str) -> Vec<String> {
    workflow
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("agenthub_features:"))
        .map(str::trim)
        .filter(|features| !features.is_empty())
        .map(str::to_owned)
        .collect()
}
