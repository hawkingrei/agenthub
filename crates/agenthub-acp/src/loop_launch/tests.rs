use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;
use crate::{
    AcpActorSkillContext, AcpEventSink, AcpPermissionService, AcpPromptDeliveryPolicy,
    AcpRuntimeLocation, AcpStream, SpawnAcpSessionRequest, spawn_acp_session,
};

const PROVIDER: &str = r#"
import json, sys
path, resume, failure = sys.argv[1:]
for line in sys.stdin:
    message = json.loads(line)
    with open(path, 'a') as log:
        log.write(json.dumps(message) + '\n')
    if 'id' not in message:
        continue
    method = message['method']
    if failure == method:
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'error':{'code':-32603,'message':'fixture rejected'}}), flush=True)
        continue
    if method == 'initialize':
        result = {'protocolVersion':1,'agentCapabilities':{'loadSession':resume == 'yes'}}
    elif method == 'session/new':
        result = {'sessionId':'fresh-session'}
    elif method == 'session/set_config_option':
        result = {'configOptions':[]}
    elif method == 'session/prompt':
        for text in ['first round', 'second round']:
            print(json.dumps({'jsonrpc':'2.0','method':'session/update','params':{'sessionId':message['params']['sessionId'],'update':{'sessionUpdate':'agent_message_chunk','content':{'type':'text','text':text}}}}), flush=True)
        result = {'stopReason':'end_turn'}
    else:
        result = {}
    print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':result}), flush=True)
"#;

#[derive(Default)]
struct Sink(Mutex<Vec<String>>);

#[async_trait::async_trait]
impl AcpEventSink for Sink {
    async fn emit_raw(&self, _: AcpStream, message: String) {
        self.0.lock().unwrap().push(message);
    }
}

struct Fixture {
    directory: PathBuf,
    child: tokio::process::Child,
    sink: Arc<Sink>,
}

impl Fixture {
    async fn new(resume: bool, failure: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("agenthub-loop-acp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let child = tokio::process::Command::new("python3")
            .args(["-u", "-c", PROVIDER])
            .arg(directory.join("requests.jsonl"))
            .arg(if resume { "yes" } else { "no" })
            .arg(failure)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        Self {
            directory,
            child,
            sink: Arc::new(Sink::default()),
        }
    }

    async fn launch(
        &mut self,
        resume: Option<&str>,
        config: AcpLoopLaunchConfig,
    ) -> anyhow::Result<crate::AcpHandle> {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let context = AcpActorSkillContext {
            team_id: Some("team".into()),
            current_run_id: Some("stable-mailbox".into()),
            actor_id: "worker".into(),
            default_channel: "default".into(),
            member_role: Some("worker".into()),
            member_skills: vec![],
            contract_version: Some(LOOP_ACTIVATION_CONTRACT_VERSION.into()),
            continuity: None,
        };
        tokio::time::timeout(
            Duration::from_secs(5),
            spawn_acp_session(SpawnAcpSessionRequest {
                provider_id: "fixture".into(),
                event_sink: self.sink.clone(),
                permissions: Arc::new(AcpPermissionService::new(db)),
                permission_review_dispatcher: None,
                agent_id: "worker".into(),
                agent_session_id: "launch".into(),
                self_reminders_enabled: true,
                loop_launch: Some(config),
                resume_session_id: resume.map(str::to_owned),
                workdir: self.directory.to_string_lossy().to_string(),
                client_info: agent_client_protocol::schema::v1::Implementation::new("fixture", "1"),
                stdout: self.child.stdout.take().unwrap(),
                stdin: self.child.stdin.take().unwrap(),
                actor_context: Some(context),
                prompt_delivery_policy: AcpPromptDeliveryPolicy::StrictFifo,
                runtime_location: AcpRuntimeLocation::LocalProcess,
            }),
        )
        .await
        .unwrap()
    }

    fn requests(&self) -> Vec<serde_json::Value> {
        std::fs::read_to_string(self.directory.join("requests.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn methods(&self) -> Vec<String> {
        self.requests()
            .iter()
            .filter_map(|request| request["method"].as_str().map(str::to_owned))
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn config(require_resume: bool) -> AcpLoopLaunchConfig {
    AcpLoopLaunchConfig {
        require_resume,
        mode_id: None,
        model_id: None,
        config: vec![],
        mcp_proxies: vec![],
        runtime_skill: None,
        skills: vec![AcpSkill {
            name: "fixture-skill".into(),
            path: "fixture".into(),
            instructions: "Launch snapshot instructions.".into(),
        }],
    }
}

#[tokio::test]
async fn loop_acp_fresh_launch_delivers_one_prompt_with_multiple_provider_updates() {
    let mut fixture = Fixture::new(false, "").await;
    let mut launch = config(false);
    launch.install_loop_runtime_skill().unwrap();
    let skill_path = launch.runtime_skill.as_ref().unwrap().path();
    let handle = fixture.launch(None, launch).await.unwrap();
    assert_eq!(handle.session_id, "fresh-session");
    handle
        .prompt_with_images_with_submission(
            "Run one activation".into(),
            vec![],
            "activation-entry".into(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let diagnostics = handle.diagnostics();
            if diagnostics.last_submission_id.as_deref() == Some("activation-entry")
                && diagnostics.active_submission_ids.is_empty()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let requests = fixture.requests();
    let prompts: Vec<_> = requests
        .iter()
        .filter(|request| request["method"] == "session/prompt")
        .collect();
    assert_eq!(prompts.len(), 1);
    let prompt = prompts[0].to_string();
    assert!(prompt.contains("stable-mailbox"));
    assert!(prompt.contains("mailbox_run_id"));
    assert!(prompt.contains("Launch snapshot instructions."));
    assert!(prompt.contains("<name>team-loop-runtime</name>"));
    assert!(prompt.contains("Recover Before Acting"));
    assert!(skill_path.is_file());
    assert!(!prompt.contains("time-trigger-set"));
    assert!(!prompt.contains("team-worker-executor"));
    assert_eq!(
        requests
            .iter()
            .find(|request| request["method"] == "session/new")
            .unwrap()["params"]["mcpServers"],
        serde_json::json!([])
    );
    let output = fixture.sink.0.lock().unwrap().join("\n");
    assert!(output.contains("first round") && output.contains("second round"));
}

#[test]
fn loop_runtime_skill_is_pinned_without_random_paths_changing_configuration() {
    let mut first = config(false);
    let mut second = config(false);
    let before = first.fingerprint_material().unwrap();
    first.install_loop_runtime_skill().unwrap();
    second.install_loop_runtime_skill().unwrap();
    assert_ne!(first.fingerprint_material().unwrap(), before);
    assert_eq!(
        first.fingerprint_material().unwrap(),
        second.fingerprint_material().unwrap()
    );
    let path = first.runtime_skill.as_ref().unwrap().path();
    assert_ne!(path, second.runtime_skill.as_ref().unwrap().path());
    let clone = first.clone();
    first.install_loop_runtime_skill().unwrap();
    assert_eq!(first.skills.len(), 2);
    drop(first);
    assert!(path.is_file());
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("Recover Before Acting")
    );
    drop(clone);
    assert!(!path.parent().unwrap().exists());
}

#[tokio::test]
async fn loop_skill_discovery_replaces_reserved_workflows_and_preserves_workspace_extensions() {
    let fixture = Fixture::new(false, "").await;
    let root = fixture.directory.join(".agents/skills");
    for kind in ManagedSkillKind::ALL {
        let name = managed_skill_name(kind);
        let directory = root.join(name);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("SKILL.md"),
            format!(
                "---\nname: {}\n---\nUntrusted replacement workflow.\n",
                name.to_ascii_uppercase()
            ),
        )
        .unwrap();
    }
    let extension = root.join("project-validation");
    std::fs::create_dir_all(&extension).unwrap();
    std::fs::write(
        extension.join("SKILL.md"),
        "---\nname: project-validation\n---\nValidate the project.\n",
    )
    .unwrap();
    let mut launch = AcpLoopLaunchConfig::resolve(&fixture.directory, false);
    assert!(
        launch
            .skills
            .iter()
            .any(|skill| skill.name == "project-validation")
    );
    assert!(!launch.skills.iter().any(|skill| {
        skill
            .instructions
            .contains("Untrusted replacement workflow.")
    }));
    launch.install_loop_runtime_skill().unwrap();
    let reserved: Vec<_> = launch
        .skills
        .iter()
        .filter(|skill| skill.name == "team-loop-runtime")
        .collect();
    assert_eq!(reserved.len(), 1);
    assert!(reserved[0].instructions.contains("Recover Before Acting"));
}

#[tokio::test]
async fn loop_acp_resume_requires_advertised_capability() {
    let mut fixture = Fixture::new(false, "").await;
    assert!(
        fixture
            .launch(Some("existing"), config(true))
            .await
            .is_err()
    );
    assert_eq!(fixture.methods(), vec!["initialize"]);
}

#[tokio::test]
async fn loop_acp_resume_preserves_provider_identity_without_creating_a_session() {
    let mut fixture = Fixture::new(true, "").await;
    let handle = fixture
        .launch(Some("existing"), config(true))
        .await
        .unwrap();
    assert_eq!(handle.session_id, "existing");
    assert_eq!(fixture.methods(), vec!["initialize", "session/load"]);
}

#[tokio::test]
async fn loop_acp_failed_resume_does_not_fall_back_to_fresh() {
    let mut fixture = Fixture::new(true, "session/load").await;
    assert!(
        fixture
            .launch(Some("existing"), config(true))
            .await
            .is_err()
    );
    assert_eq!(fixture.methods(), vec!["initialize", "session/load"]);
}

#[tokio::test]
async fn loop_acp_required_profile_rejection_fails_startup() {
    let mut fixture = Fixture::new(false, "session/set_config_option").await;
    let mut launch = config(false);
    launch.model_id = Some("required-model".into());
    assert!(fixture.launch(None, launch).await.is_err());
    assert_eq!(
        fixture.methods(),
        vec!["initialize", "session/new", "session/set_config_option"]
    );
}

#[tokio::test]
async fn loop_acp_proxy_descriptors_are_local_and_survive_fresh_and_resume_launch() {
    for resume in [false, true] {
        let mut fixture = Fixture::new(true, "").await;
        let mut launch = config(resume);
        let credential = fixture.directory.join("activation-credential.json");
        launch
            .add_mcp_proxy(
                Path::new("/usr/bin/agenthub"),
                &credential,
                "nowledge-mem",
                &"a".repeat(64),
            )
            .unwrap();
        let fingerprint = launch.fingerprint_material().unwrap();
        assert!(
            !String::from_utf8(fingerprint.clone())
                .unwrap()
                .contains("activation-credential")
        );
        let mut rotated_path = config(resume);
        rotated_path
            .add_mcp_proxy(
                Path::new("/usr/bin/agenthub"),
                Path::new("/private/other-credential.json"),
                "nowledge-mem",
                &"a".repeat(64),
            )
            .unwrap();
        assert_eq!(fingerprint, rotated_path.fingerprint_material().unwrap());
        fixture
            .launch(resume.then_some("existing"), launch)
            .await
            .unwrap();
        let requests = fixture.requests();
        let request = requests
            .iter()
            .find(|request| {
                request["method"]
                    == if resume {
                        "session/load"
                    } else {
                        "session/new"
                    }
            })
            .unwrap();
        let server = &request["params"]["mcpServers"][0];
        assert_eq!(server["command"], "/usr/bin/agenthub");
        assert_eq!(
            server["args"],
            serde_json::json!(["mcp-proxy", "--server-id", "nowledge-mem"])
        );
        assert_eq!(
            server["env"],
            serde_json::json!([{"name":"AGENTHUB_LOOP_CREDENTIAL_FILE", "value":credential}])
        );
        assert!(server.get("url").is_none() && server.get("headers").is_none());
    }
}

#[test]
fn loop_acp_proxy_capacity_fits_registered_apps_and_mem_with_a_hard_bound() {
    let mut launch = config(false);
    for index in 0..32 {
        launch
            .add_mcp_proxy(
                Path::new("/usr/bin/agenthub"),
                Path::new("/private/credentials"),
                &format!("app-{index}"),
                &"a".repeat(64),
            )
            .unwrap();
    }
    assert_eq!(launch.mcp_servers().len(), 32);
    assert!(
        launch
            .add_mcp_proxy(
                Path::new("/usr/bin/agenthub"),
                Path::new("/private/credentials"),
                "one-too-many",
                &"a".repeat(64)
            )
            .is_err()
    );
}
