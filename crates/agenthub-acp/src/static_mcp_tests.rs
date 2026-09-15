use std::{path::PathBuf, process::Stdio, sync::atomic::AtomicUsize, time::Duration};

use super::*;

const SERVER: &str = r#"
import json, os, sys
assert os.environ['STATIC_MCP_VALUE'] == 'preserved-environment'
for line in sys.stdin:
    request = json.loads(line)
    if 'id' not in request:
        assert request['method'] == 'notifications/initialized'
        continue
    method = request['method']
    if method == 'initialize':
        result = {'protocolVersion':'2025-11-25','capabilities':{'tools':{}},'serverInfo':{'name':'native-static','version':'1'}}
    elif method == 'tools/list':
        result = {'tools':[{'name':'legacy_echo','inputSchema':{'type':'object','properties':{'message':{'type':'string'}}}}]}
    elif method == 'tools/call':
        assert request['params'] == {'name':'legacy_echo','arguments':{'message':'unchanged'}}
        result = {'content':[{'type':'text','text':'from-native-static-server'}],'extension':{'preserved':True}}
    else:
        raise AssertionError('unexpected MCP method')
    print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result}), flush=True)
"#;

const PROVIDER: &str = r#"
import json, os, subprocess, sys
directory, http, mode = sys.argv[1:]
for line in sys.stdin:
    request = json.loads(line)
    if 'id' not in request:
        continue
    method = request['method']
    if method == 'initialize':
        result = {'protocolVersion':1,'agentCapabilities':{'loadSession':True,'mcpCapabilities':{'http':http == 'yes'}}}
    elif method in ['session/new', 'session/load']:
        servers = {server['name']:server for server in request['params']['mcpServers']}
        if mode == 'loop':
            assert servers == {}
        else:
            assert set(servers) == ({'native-stdio','native-http'} if http == 'yes' else {'native-stdio'})
            server = servers['native-stdio']
            assert server['command'] == 'python3'
            assert server['args'] == ['-u', os.path.join(directory, 'mcp.py')]
            assert server['env'] == [{'name':'STATIC_MCP_VALUE','value':'preserved-environment'}]
            if http == 'yes':
                assert servers['native-http'] == {'type':'http','name':'native-http','url':'http://127.0.0.1:1/static-fixture','headers':[{'name':'X-Static-Fixture','value':'preserved-header'}]}
            env = dict(os.environ)
            env.update({item['name']:item['value'] for item in server['env']})
            with subprocess.Popen([server['command']] + server['args'], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True) as child:
                def call(message):
                    child.stdin.write(json.dumps(message) + '\n')
                    child.stdin.flush()
                    return json.loads(child.stdout.readline())
                initialized = call({'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'static-provider','version':'1'}}})
                assert initialized['result']['serverInfo']['name'] == 'native-static'
                child.stdin.write(json.dumps({'jsonrpc':'2.0','method':'notifications/initialized'}) + '\n')
                assert call({'jsonrpc':'2.0','id':2,'method':'tools/list'})['result']['tools'][0]['name'] == 'legacy_echo'
                response = call({'jsonrpc':'2.0','id':3,'method':'tools/call','params':{'name':'legacy_echo','arguments':{'message':'unchanged'}}})
                assert response['result'] == {'content':[{'type':'text','text':'from-native-static-server'}],'extension':{'preserved':True}}
                child.stdin.close()
                assert child.wait(timeout=5) == 0
        with open(os.path.join(directory, 'observed.json'), 'w') as output:
            json.dump({'method':method,'servers':sorted(servers),'native_call':mode != 'loop'}, output)
        result = {'sessionId':'static-fresh'} if method == 'session/new' else {}
    else:
        result = {}
    print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':result}), flush=True)
"#;

struct Sink;

#[async_trait::async_trait]
impl AcpEventSink for Sink {
    async fn emit_raw(&self, _: AcpStream, _: String) {}
}

struct Fixture {
    directory: PathBuf,
    child: tokio::process::Child,
}

impl Fixture {
    async fn new(http: bool, loop_mode: bool) -> Self {
        let directory =
            std::env::temp_dir().join(format!("agenthub-static-mcp-{}", Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("mcp.py"), SERVER).unwrap();
        fs::write(directory.join("mcp.json"), serde_json::to_vec(&serde_json::json!({"mcpServers":{
            "native-stdio":{"command":"python3","args":["-u",directory.join("mcp.py")],"env":{"STATIC_MCP_VALUE":"preserved-environment"}},
            "native-http":{"url":"http://127.0.0.1:1/static-fixture","headers":{"X-Static-Fixture":"preserved-header"}}
        }})).unwrap()).unwrap();
        let child = tokio::process::Command::new("python3")
            .args(["-u", "-c", PROVIDER])
            .arg(&directory)
            .arg(if http { "yes" } else { "no" })
            .arg(if loop_mode { "loop" } else { "legacy" })
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        Self { directory, child }
    }

    async fn launch(&mut self, resume: bool, loop_mode: bool) -> AcpHandle {
        let path = self.directory.join("mcp.json");
        let loads = Arc::new(AtomicUsize::new(0));
        let observed_loads = loads.clone();
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let handle = tokio::time::timeout(
            Duration::from_secs(10),
            spawn_acp_session_with_static_mcp(
                SpawnAcpSessionRequest {
                    provider_id: "static-fixture".into(),
                    event_sink: Arc::new(Sink),
                    permissions: Arc::new(AcpPermissionService::new(db)),
                    permission_review_dispatcher: None,
                    agent_id: "static-agent".into(),
                    agent_session_id: "static-launch".into(),
                    self_reminders_enabled: false,
                    loop_launch: loop_mode
                        .then(|| AcpLoopLaunchConfig::resolve(&self.directory, resume)),
                    resume_session_id: resume.then(|| "static-existing".into()),
                    workdir: self.directory.to_string_lossy().into_owned(),
                    client_info: Implementation::new("static-fixture", "1"),
                    stdout: self.child.stdout.take().unwrap(),
                    stdin: self.child.stdin.take().unwrap(),
                    actor_context: None,
                    prompt_delivery_policy: AcpPromptDeliveryPolicy::StrictFifo,
                    runtime_location: AcpRuntimeLocation::LocalProcess,
                },
                move || {
                    observed_loads.fetch_add(1, Ordering::SeqCst);
                    load_mcp_servers_from_path(&path)
                },
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(loads.load(Ordering::SeqCst), usize::from(!loop_mode));
        handle
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[tokio::test]
async fn legacy_static_mcp_launch_preserves_native_servers_for_fresh_and_resumed_sessions() {
    for resume in [false, true] {
        for http in [false, true] {
            let mut fixture = Fixture::new(http, false).await;
            let handle = fixture.launch(resume, false).await;
            assert_eq!(
                handle.session_id,
                if resume {
                    "static-existing"
                } else {
                    "static-fresh"
                }
            );
            let observed: Value =
                serde_json::from_slice(&fs::read(fixture.directory.join("observed.json")).unwrap())
                    .unwrap();
            assert_eq!(observed["native_call"], true);
            assert_eq!(
                observed["method"],
                if resume {
                    "session/load"
                } else {
                    "session/new"
                }
            );
            assert_eq!(
                observed["servers"],
                if http {
                    serde_json::json!(["native-http", "native-stdio"])
                } else {
                    serde_json::json!(["native-stdio"])
                }
            );
            drop(handle);
        }
    }
}

#[tokio::test]
async fn loop_mcp_launch_does_not_load_ambient_static_servers_on_fresh_or_resume() {
    for resume in [false, true] {
        let mut fixture = Fixture::new(true, true).await;
        let handle = fixture.launch(resume, true).await;
        let observed: Value =
            serde_json::from_slice(&fs::read(fixture.directory.join("observed.json")).unwrap())
                .unwrap();
        assert_eq!(observed["native_call"], false);
        assert_eq!(observed["servers"], serde_json::json!([]));
        drop(handle);
    }
}
