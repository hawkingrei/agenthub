// Fake ACP provider exercises the real local shim.
pub(super) const SCRIPT: &str = r#"
import json
import os
import subprocess
from pathlib import Path

shim = None
available = False
private_keys = ['TEST_APP_TOKEN', 'TEST_UNUSED_APP_TOKEN', 'TEST_REVOKED_APP_TOKEN',
                'TEST_EVENT_KEY', 'TEST_OLD_EVENT_KEY', 'TEST_UNUSED_EVENT_KEY', 'TEST_REVOKED_EVENT_KEY']


def call(request_id, method, params):
    shim.stdin.write(json.dumps({'jsonrpc': '2.0', 'id': request_id, 'method': method, 'params': params}) + '\n')
    shim.stdin.flush()
    return json.loads(shim.stdout.readline())


def start(params, log_path):
    global shim, available
    assert all(key not in os.environ for key in private_keys)
    assert len(params['mcpServers']) == 1
    server = params['mcpServers'][0]
    assert server['name'].startswith('app-')
    assert server['args'] == ['mcp-proxy', '--server-id', server['name']]
    assert 'url' not in server and 'headers' not in server
    env = dict(os.environ)
    env.update({item['name']: item['value'] for item in server['env']})
    shim = subprocess.Popen([server['command']] + server['args'], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    with open('/proc/' + str(shim.pid) + '/environ', 'rb') as environ:
        inherited = environ.read().split(b'\0')
    assert all(not any(item.startswith(key.encode() + b'=') for item in inherited) for key in private_keys)
    result = call(1, 'initialize', {'protocolVersion': '2025-11-25', 'capabilities': {}, 'clientInfo': {'name': 'app-fixture', 'version': '1'}})
    if Path('app-unavailable').exists():
        assert 'error' in result
        assert len(json.dumps(result)) < 1024
        return
    assert result['result']['protocolVersion'] == '2025-11-25'
    shim.stdin.write(json.dumps({'jsonrpc': '2.0', 'method': 'notifications/initialized'}) + '\n')
    tools = call(2, 'tools/list', {})['result']['tools']
    assert [tool['name'] for tool in tools] == ['write']
    available = True


def work(actor, log_path):
    if available:
        denied = call(3, 'tools/call', {'name': 'write', 'arguments': {'body': 17}})
        assert 'error' in denied
        result = call(4, 'tools/call', {'name': 'write', 'arguments': {'body': 'native-app-input'}})
        assert result['result']['structuredContent']['written'] is True
    shim.stdin.close()
    assert shim.wait(timeout=5) == 0
    with open(log_path, 'a') as log:
        log.write(json.dumps({'app_available': available}) + '\n')
    # Independent actor recovery and structured finish still run after upstream unavailability.
    assert actor('loop-context')['activation']['state'] == 'running'
"#;
