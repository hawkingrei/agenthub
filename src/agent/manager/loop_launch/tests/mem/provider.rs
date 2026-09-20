// Fake ACP behavior uses the real shim and native discovery. Production policy stays in the daemon.
pub(crate) const SCRIPT: &str = r#"
import json
import os
import subprocess
import uuid
from pathlib import Path

shim = None
tools = {}

def log_event(path, event):
    with open(path, 'a') as log:
        log.write(json.dumps(event) + '\n')

def call(method, params):
    request_id = str(uuid.uuid4())
    shim.stdin.write(json.dumps({'jsonrpc':'2.0', 'id':request_id, 'method':method, 'params':params}) + '\n')
    shim.stdin.flush()
    while True:
        response = json.loads(shim.stdout.readline())
        if response.get('id') == request_id:
            return response

def close():
    global shim
    if shim is not None:
        shim.stdin.close()
        assert shim.wait(timeout=5) == 0
        shim = None

def start(params, log_path):
    global shim, tools
    tools = {}
    servers = params['mcpServers']
    if not servers:
        return
    assert len(servers) == 1
    server = servers[0]
    assert server['args'] == ['mcp-proxy', '--server-id', 'nowledge-mem']
    assert 'url' not in server and 'headers' not in server
    env = dict(os.environ)
    env.update({item['name']:item['value'] for item in server['env']})
    assert 'TEST_MEM_UPSTREAM_KEY' not in env and 'NMEM_API_KEY' not in env
    shim = subprocess.Popen([server['command']] + server['args'], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    response = call('initialize', {'protocolVersion':'2025-06-18', 'capabilities':{}, 'clientInfo':{'name':'mem-fixture-acp', 'version':'1'}})
    if 'error' in response:
        log_event(log_path, {'mcp_startup_failed':True})
        close()
        return
    assert response['result']['protocolVersion'] == '2025-06-18'
    shim.stdin.write(json.dumps({'jsonrpc':'2.0', 'method':'notifications/initialized'}) + '\n')
    cursor = None
    while True:
        page = call('tools/list', {} if cursor is None else {'cursor':cursor})
        if 'error' in page:
            log_event(log_path, {'mcp_discovery_failed':True})
            close()
            return
        for tool in page['result']['tools']:
            tools[tool['name']] = tool
        cursor = page['result'].get('nextCursor')
        if cursor is None:
            break
    log_event(log_path, {'mcp_started':True})

def work(params, actor, log_path):
    prompt = ''.join(block.get('text', '') for block in params['prompt'])
    log_event(log_path, {'context_prompt':prompt})
    task_id = Path('local-task-id').read_text()
    task = actor('team-task-note', '--task-id', task_id, '--kind', 'result', '--text', 'Independent local progress is durable')
    log_event(log_path, {'local_task':task})
    if Path('learning-case').exists():
        retain_learning(actor, task_id, log_path)
    close()

def retain_learning(actor, task_id, log_path):
    activation = actor('loop-context')['activation']
    actor('team-task-show', '--task-id', task_id)
    legacy = Path('.agenthubmemory/legacy.md').read_text()
    assert 'Original retry identity' in legacy
    for name in ['memory_search', 'read_working_memory', 'thread_search', 'search_source_chunks']:
        properties = tools[name]['inputSchema']['properties']
        args = {'query':'original retry identity'} if 'query' in properties else {}
        response = call('tools/call', {'name':name, 'arguments':args})
        log_event(log_path, {'retrieval':name, 'response':response})
    payload_path = Path('selected-learning.json')
    if payload_path.exists():
        # Deliberately attempt an unchanged uncertain write to exercise daemon replay enforcement.
        args = json.loads(payload_path.read_text())
    else:
        artifact = Path('artifacts/retry-evidence.md')
        artifact.parent.mkdir(exist_ok=True)
        artifact.write_text('Verified: retries retain the original evidence identity.\n')
        provenance = json.dumps({'task_id':task_id, 'activation_id':activation['id'], 'artifacts':[str(artifact), '.agenthubmemory/legacy.md']}, sort_keys=True)
        args = {'content':'Retain the original evidence identity when reconciling an uncertain write.'}
        properties = tools['memory_add']['inputSchema']['properties']
        if 'source_grounding' in properties:
            args['source_grounding'] = provenance
        else:
            args['content'] += '\n\nSource evidence: ' + provenance
        payload_path.write_text(json.dumps(args, sort_keys=True))
    response = call('tools/call', {'name':'memory_add', 'arguments':args})
    log_event(log_path, {'learning_response':response, 'activation_id':activation['id']})
    if 'error' in response:
        receipt = {'status':'unresolved', 'payload':'selected-learning.json'}
    else:
        native = json.loads(response['result']['content'][0]['text'])
        assert native['status'] == 'created'
        receipt = {'status':'retained', 'memory_id':native['id'], 'payload':'selected-learning.json'}
    actor('team-task-note', '--task-id', task_id, '--kind', 'result', '--text', json.dumps(receipt, sort_keys=True))
"#;
