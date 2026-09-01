#![recursion_limit = "512"]

mod acp;
mod actor_cli;
mod actor_runtime_env;
mod agent;
#[cfg(test)]
mod agenthub_binary;
mod api;
mod app;
mod auth;
mod cli;
mod cli_error;
mod daemon_binary;
mod daemon_instance;
mod daemon_tasks;
mod diagnostics;
mod doctor_cli;
mod init_cli;
pub use agenthub_config as config;
pub use agenthub_db as db;
mod internal;
mod linkers;
pub mod message_body_store;
mod migrate_cli;
pub mod object_upload;
pub use agenthub_config::path_utils;
mod push;
mod sse;
mod state;
mod team;
mod web;

pub use app::{run, run_daemon};
pub use cli_error::report_cli_error;

#[cfg(test)]
mod release_feature_tests {
    use toml::Value;

    const ROOT_CARGO_TOML: &str = include_str!("../Cargo.toml");
    const CARGO_LOCK: &str = include_str!("../Cargo.lock");
    const OBJECT_STORE_CARGO_TOML: &str =
        include_str!("../crates/agenthub-object-store/Cargo.toml");
    const ACP_ADAPTER_CARGO_TOML: &str = include_str!("../crates/agenthub-acp-adapter/Cargo.toml");
    const DAEMON_CARGO_TOML: &str = include_str!("../crates/agenthub-daemon/Cargo.toml");
    const CODEX_RUNTIME_CARGO_TOML: &str = include_str!("../agenthub-codex-acp/Cargo.toml");
    const CODEX_STDIO_APP_SERVER_SOURCE: &str =
        include_str!("../agenthub-codex-acp/src/stdio_app_server.rs");
    const BAZEL_MODULE: &str = include_str!("../MODULE.bazel");
    const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release.yml");
    const RELEASE_PREBUILD_WORKFLOW: &str =
        include_str!("../.github/workflows/release-prebuild.yml");
    const ROOT_MAKEFILE: &str = include_str!("../Makefile");
    const DEB_PACKAGE_SCRIPT: &str = include_str!("../build/deb/package.sh");
    const SYSTEMD_UNIT: &str = include_str!("../build/deb/agenthub.service");
    const USER_INSTALLATION_DOC: &str =
        include_str!("../userdocs/docs/getting-started/installation.md");
    const USER_TROUBLESHOOTING_DOC: &str =
        include_str!("../userdocs/docs/operations/troubleshooting.md");
    const TODO_MD: &str = include_str!("../docs/todo.md");
    const S3_RELEASE_JOURNAL: &str =
        include_str!("../docs/journal/2026-08-08-object-store-s3-release-enablement.md");

    #[test]
    fn official_release_includes_opendal_s3_without_changing_defaults() {
        let root_manifest: Value = toml::from_str(ROOT_CARGO_TOML).expect("parse root Cargo.toml");
        let object_store_manifest: Value =
            toml::from_str(OBJECT_STORE_CARGO_TOML).expect("parse object-store Cargo.toml");

        assert_eq!(
            root_manifest["features"]["default"],
            Value::Array(Vec::new()),
            "root default features must stay empty"
        );
        assert_eq!(
            object_store_manifest["features"]["default"],
            Value::Array(Vec::new()),
            "object-store default features must stay empty"
        );
        assert!(
            root_manifest["features"]["object-store-s3"].is_array(),
            "root manifest must keep S3 behind the explicit object-store-s3 feature"
        );
        assert!(
            object_store_manifest["features"]["s3"].is_array(),
            "object-store manifest must keep S3 behind the explicit s3 feature"
        );
        assert_eq!(
            root_manifest["features"]["object-store-s3"],
            Value::Array(vec![Value::String("agenthub-object-store/s3".to_string())]),
            "root S3 feature must stay as the explicit bridge to agenthub-object-store/s3"
        );

        for (name, workflow) in [
            ("release.yml", RELEASE_WORKFLOW),
            ("release-prebuild.yml", RELEASE_PREBUILD_WORKFLOW),
        ] {
            assert!(
                !workflow.contains("--all-features"),
                "{name} must keep the reviewed release feature closure explicit"
            );
            let feature_rows = workflow
                .lines()
                .map(str::trim)
                .filter_map(|line| line.strip_prefix("agenthub_features:"))
                .collect::<Vec<_>>();
            assert!(
                !feature_rows.is_empty(),
                "{name} must declare at least one agenthub release feature row"
            );
            assert!(
                feature_rows.iter().all(|features| {
                    features
                        .split(',')
                        .map(str::trim)
                        .any(|feature| feature == "object-store-s3")
                }),
                "{name} must compile OpenDAL S3 support into every agenthub release artifact"
            );
            assert!(
                !workflow.contains("agenthub-object-store/s3"),
                "{name} must enable S3 through the root object-store-s3 feature"
            );
        }
        assert!(
            !TODO_MD.contains("- [ ] `P1` Verify OpenDAL S3 release artifacts"),
            "remove the S3 artifact gate after published-binary evidence lands"
        );
        assert!(
            S3_RELEASE_JOURNAL.contains("31259043337")
                && S3_RELEASE_JOURNAL.contains("9022659373")
                && S3_RELEASE_JOURNAL
                    .contains("4438513d8a4298c30697dcf1d3e50f869640cb9cfb66930543e73ed627b3ce24"),
            "journal must retain the official workflow, artifact, and archive evidence"
        );
        assert!(
            S3_RELEASE_JOURNAL.contains("cleanup_attempts_total = 1")
                && S3_RELEASE_JOURNAL.contains("cleanup_successes_total = 1")
                && S3_RELEASE_JOURNAL.contains("runtime backend"),
            "journal must retain the compensating-delete and runtime-default boundaries"
        );
    }

    #[test]
    fn release_workflow_keeps_partial_asset_publication_path_open() {
        assert!(
            RELEASE_WORKFLOW.contains("fail-fast: false"),
            "release build matrix must keep fail-fast disabled so successful targets can upload artifacts"
        );
        assert!(
            RELEASE_WORKFLOW.contains("name: Create Release"),
            "release workflow must keep a Create Release job"
        );
        assert!(
            RELEASE_WORKFLOW.contains("needs: [build, publish_npm]"),
            "Create Release must wait for build and npm jobs before publishing collected artifacts"
        );
        assert!(
            RELEASE_WORKFLOW.contains(
                "if: ${{ always() && !cancelled() && (needs.publish_npm.result == 'success' || needs.publish_npm.result == 'skipped') }}"
            ),
            "Create Release must not require the build matrix to be fully successful before collecting partial artifacts"
        );
        assert!(
            RELEASE_WORKFLOW.contains("pattern: release-*"),
            "Create Release must download successful matrix artifacts by release-* pattern"
        );
        assert!(
            RELEASE_WORKFLOW.contains("merge-multiple: true"),
            "Create Release must merge matrix artifacts before publishing"
        );
        assert!(
            RELEASE_WORKFLOW
                .contains("No binary release assets were produced by the build matrix."),
            "Create Release must fail closed when every binary target failed"
        );
        assert!(
            RELEASE_WORKFLOW.contains("One or more release targets failed in the build matrix."),
            "release body must warn when publishing a partial build result"
        );
        assert!(
            TODO_MD.contains("- [ ] `P1` Verify preview release partial-asset behavior"),
            "keep the preview partial-asset TODO open until a real preview run proves partial publication"
        );
        assert!(
            TODO_MD
                .contains("successful binary assets publish when one release matrix target fails"),
            "TODO should still require direct preview release evidence before closure"
        );
    }

    #[test]
    fn release_builds_exactly_the_cli_and_daemon_entrypoints() {
        let adapter_manifest: Value =
            toml::from_str(ACP_ADAPTER_CARGO_TOML).expect("parse ACP adapter Cargo.toml");
        let daemon_manifest: Value =
            toml::from_str(DAEMON_CARGO_TOML).expect("parse daemon Cargo.toml");
        let codex_runtime_manifest: Value =
            toml::from_str(CODEX_RUNTIME_CARGO_TOML).expect("parse Codex runtime Cargo.toml");
        let adapter_dependencies = adapter_manifest["dependencies"]
            .as_table()
            .expect("ACP adapter dependencies");
        assert!(
            adapter_dependencies.get("agenthub-codex-acp").is_none(),
            "agenthub-acp-adapter must not depend on the legacy agenthub-codex-acp package"
        );
        assert!(
            adapter_dependencies
                .get("agenthub-codex-acp-runtime")
                .is_some(),
            "agenthub-acp-adapter should depend on the non-legacy Codex ACP runtime package"
        );
        assert!(
            adapter_manifest.get("bin").is_none(),
            "ACP adapter must remain a library inside agenthubd"
        );
        assert!(
            codex_runtime_manifest.get("bin").is_none(),
            "Codex ACP runtime must remain a library inside agenthubd"
        );
        assert_eq!(
            daemon_manifest["bin"][0]["name"],
            Value::String("agenthubd".to_string()),
            "daemon package must expose the canonical agenthubd binary"
        );
        for (name, workflow) in [
            ("release.yml", RELEASE_WORKFLOW),
            ("release-prebuild.yml", RELEASE_PREBUILD_WORKFLOW),
        ] {
            assert!(
                workflow.contains("package_binary \"agenthub\""),
                "{name} must package the canonical agenthub binary"
            );
            assert!(
                workflow.contains("package_binary \"agenthubd\""),
                "{name} must package the canonical agenthubd binary"
            );
            assert!(
                !workflow.contains("package_binary \"agenthub-acp\"")
                    && !workflow.contains("package_binary \"agenthub-codex-acp\""),
                "{name} must not package legacy ACP entrypoints"
            );
        }
        assert!(
            RELEASE_WORKFLOW.contains("daemon_archive_name")
                && RELEASE_WORKFLOW.contains("${stage_dir}/bin/${binary_name}")
                && !RELEASE_WORKFLOW.contains("codex-code-mode-host"),
            "npm staging must install both AgentHub binaries without a Codex helper"
        );
        assert!(
            DEB_PACKAGE_SCRIPT.contains("for binary in agenthub agenthubd")
                && !DEB_PACKAGE_SCRIPT.contains("codex-code-mode-host")
                && !DEB_PACKAGE_SCRIPT.contains("usr/bin/agenthub-acp"),
            "Debian packages must install only AgentHub-owned executables"
        );
        for (name, workflow) in [
            ("release.yml", RELEASE_WORKFLOW),
            ("release-prebuild.yml", RELEASE_PREBUILD_WORKFLOW),
        ] {
            assert!(
                !workflow.contains("codex-code-mode-host")
                    && !workflow.contains("fetch-code-mode-host"),
                "{name} must not fetch or package Codex helper artifacts"
            );
        }
        assert!(
            daemon_manifest["dependencies"].get("codex-arg0").is_none()
                && codex_runtime_manifest["dependencies"]
                    .get("codex-app-server-client")
                    .is_none(),
            "AgentHub must delegate Codex helper and app-server process ownership to installed Codex"
        );
        assert!(
            SYSTEMD_UNIT.contains("ExecStart=/usr/bin/agenthubd"),
            "systemd must execute the daemon directly"
        );
    }

    #[test]
    fn source_and_portable_installs_require_external_codex() {
        assert!(
            ROOT_MAKEFILE.contains("build:\n")
                && ROOT_MAKEFILE.contains("run-server: build-web")
                && ROOT_MAKEFILE.contains("export CARGO_TARGET_DIR")
                && !ROOT_MAKEFILE.contains("code-mode-host"),
            "supported source build and run paths must build only AgentHub-owned executables"
        );
        assert!(
            USER_INSTALLATION_DOC.contains("@openai/codex@0.150.1")
                && USER_INSTALLATION_DOC.contains("runtime_binary")
                && !USER_INSTALLATION_DOC.contains("agenthubd-*-${TARGET}/codex-code-mode-host"),
            "portable install instructions must require the supported external Codex runtime"
        );
    }

    #[test]
    fn installed_codex_version_matches_protocol_and_bazel_pins() {
        let runtime_manifest: Value =
            toml::from_str(CODEX_RUNTIME_CARGO_TOML).expect("parse Codex runtime Cargo.toml");
        let dependencies = runtime_manifest["dependencies"]
            .as_table()
            .expect("Codex runtime dependencies");
        let protocol_rev = dependencies["codex-app-server-protocol"]["rev"]
            .as_str()
            .expect("Codex app-server protocol revision");
        assert!(
            dependencies
                .iter()
                .filter(|(name, _)| name.starts_with("codex-"))
                .all(|(_, dependency)| dependency["rev"].as_str() == Some(protocol_rev)),
            "all direct Codex dependencies must use the reviewed app-server protocol revision"
        );
        assert!(
            BAZEL_MODULE.contains(&format!("commit = \"{protocol_rev}\"")),
            "Bazel must use the same official Codex revision as Cargo"
        );

        let cargo_lock: Value = toml::from_str(CARGO_LOCK).expect("parse Cargo.lock");
        let protocol_version = cargo_lock["package"]
            .as_array()
            .expect("Cargo.lock packages")
            .iter()
            .find(|package| package["name"].as_str() == Some("codex-app-server-protocol"))
            .and_then(|package| package["version"].as_str())
            .expect("Codex app-server protocol version");
        assert!(
            CODEX_STDIO_APP_SERVER_SOURCE.contains(&format!(
                "SUPPORTED_CODEX_VERSION: &str = \"{protocol_version}\""
            )),
            "installed Codex preflight version must match the compiled app-server protocol"
        );
        assert!(
            USER_INSTALLATION_DOC.contains(&format!("@openai/codex@{protocol_version}")),
            "installation docs must name the supported official Codex version"
        );
    }

    #[test]
    fn provider_timeout_recovery_uses_service_loaded_proxy_config() {
        assert!(
            USER_TROUBLESHOOTING_DOC.contains("Falling back from WebSockets to HTTPS transport.")
                && USER_TROUBLESHOOTING_DOC.contains("request timed out")
                && USER_TROUBLESHOOTING_DOC.contains("[proxy]")
                && USER_TROUBLESHOOTING_DOC
                    .contains("Persisted mailbox rows are diagnostic evidence"),
            "troubleshooting must distinguish provider egress failure from mailbox fan-out"
        );
    }
}
