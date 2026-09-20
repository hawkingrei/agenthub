#!/usr/bin/env python3
"""Opt-in ACP qualification with real binaries and a local Responses fixture.

No provider account is used. The child gets an isolated Codex profile. This proves
transport, configuration, native execution, persistence and cancellation, not model quality.
"""

import argparse
import contextlib
import http.server
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time


class Model(http.server.BaseHTTPRequestHandler):
    rounds = []

    def log_message(self, *_):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        last_user = max(
            i for i, item in enumerate(body["input"]) if item.get("role") == "user"
        )
        current = body["input"][last_user:]
        permission = "permission-probe" in json.dumps(current[0])
        self.rounds.append(permission)
        index = len(self.rounds)
        if any(item["type"] == "function_call_output" for item in current):
            item = {
                "type": "message", "role": "assistant", "id": f"message-{index}",
                "content": [{"type": "output_text", "text": "Native command complete."}],
            }
        else:
            names = {tool.get("name") for tool in body["tools"]}
            name = next(n for n in ["exec_command", "shell_command"] if n in names)
            target = "must-not-exist" if permission else "native-result"
            command = f"printf accepted >> {target}"
            arguments = (
                {"cmd": command, "yield_time_ms": 1000}
                if name == "exec_command" else
                {"command": command, "timeout_ms": 10000}
            )
            if permission:
                arguments.update(
                    sandbox_permissions="require_escalated",
                    justification="Allow the isolated acceptance marker write?",
                )
            item = {
                "type": "function_call", "call_id": f"call-{index}", "name": name,
                "arguments": json.dumps(arguments),
            }
        events = [
            {"type": "response.created", "response": {"id": f"response-{index}"}},
            {"type": "response.output_item.done", "item": item},
            {"type": "response.completed", "response": {
                "id": f"response-{index}",
                "usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2},
            }},
        ]
        encoded = "".join(
            f"event: {event['type']}\ndata: {json.dumps(event)}\n\n" for event in events
        ).encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)


class Client:
    def __init__(self, command, root):
        self.error = (root / "adapter.stderr").open("a")
        self.process = subprocess.Popen(
            command, cwd=root, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=self.error, text=True,
            env=dict(os.environ, CODEX_HOME=str(root / "profile"),
                     NO_PROXY="127.0.0.1,localhost,::1", no_proxy="127.0.0.1,localhost,::1"),
        )
        self.messages = queue.Queue()
        self.next_id = 0
        self.updates = []
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        for line in self.process.stdout:
            self.messages.put(json.loads(line))
        self.messages.put({"exited": True})

    def send(self, message):
        self.process.stdin.write(json.dumps(dict(jsonrpc="2.0", **message)) + "\n")
        self.process.stdin.flush()

    def start(self, method, params):
        self.next_id += 1
        self.send({"id": self.next_id, "method": method, "params": params})
        return self.next_id

    def wait(self, predicate):
        deadline = time.monotonic() + 60
        while True:
            message = self.messages.get(timeout=max(0, deadline - time.monotonic()))
            assert "exited" not in message, "Adapter exited; inspect adapter.stderr"
            if predicate(message):
                return message
            self.updates.append(message)
            assert message.get("method") != "session/request_permission", message

    def result(self, request_id):
        result = self.wait(lambda message: message.get("id") == request_id
                           and ("result" in message or "error" in message))
        assert "error" not in result, result
        return result["result"]

    def rpc(self, method, params):
        return self.result(self.start(method, params))

    def initialize(self):
        result = self.rpc("initialize", {
            "protocolVersion": 1, "clientCapabilities": {},
            "clientInfo": {"name": "runtime-qualification", "version": "1"},
        })
        assert result["agentCapabilities"]["loadSession"]

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=10)
        self.error.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--adapter", required=True, type=Path)
    parser.add_argument("--codex", required=True, type=Path)
    args = parser.parse_args()
    version = subprocess.check_output([str(args.codex), "--version"], text=True).strip()
    assert version == "codex-cli 0.150.1", version
    root = Path(tempfile.mkdtemp(prefix="acp-runtime-qualification-"))
    (root / "profile").mkdir()
    print(f"Artifacts: {root}", flush=True)
    with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Model) as server:
        threading.Thread(target=server.serve_forever, daemon=True).start()
        overrides = [
            'model="gpt-5.4-mini"', 'model_provider="qualification"',
            'model_providers.qualification={name="Local qualification",'
            f'base_url="http://127.0.0.1:{server.server_port}/v1",'
            'wire_api="responses",requires_openai_auth=false}',
            *[f"features.{feature}=false" for feature in [
                "code_mode", "code_mode_only", "plugins", "apps", "memories",
                "responses_websockets", "responses_websockets_v2",
            ]],
        ]
        command = [str(args.adapter.resolve()), "acp", "codex", "--codex-binary", str(args.codex.resolve())]
        for value in overrides:
            command.extend(["-c", value])
        with contextlib.closing(Client(command, root)) as client:
            client.initialize()
            session = client.rpc("session/new", {"cwd": str(root), "mcpServers": []})["sessionId"]
            client.rpc("session/set_mode", {"sessionId": session, "modeId": "full-access"})
            for key, value in [("model", "gpt-5.4-mini"), ("reasoning_effort", "low")]:
                client.rpc("session/set_config_option", {"sessionId": session, "configId": key, "value": value})
            assert client.rpc("session/prompt", {"sessionId": session, "prompt": [
                {"type": "text", "text": "Run the native command fixture."},
            ]})["stopReason"] == "end_turn"
        assert (root / "native-result").read_text() == "accepted"
        with contextlib.closing(Client(command, root)) as client:
            client.initialize()
            client.rpc("session/load", {"sessionId": session, "cwd": str(root), "mcpServers": []})
            assert "Run the native command fixture." in json.dumps(client.updates), \
                "The original user message must replay on load"
            client.rpc("session/prompt", {"sessionId": session, "prompt": [
                {"type": "text", "text": "Run another native command after resume."},
            ]})
            assert (root / "native-result").read_text() == "acceptedaccepted"
            client.rpc("session/set_mode", {"sessionId": session, "modeId": "read-only"})
            pending = client.start("session/prompt", {"sessionId": session, "prompt": [
                {"type": "text", "text": "permission-probe: request approval for the marker."},
            ]})
            permission = client.wait(lambda message: message.get("method") == "session/request_permission")
            assert not (root / "must-not-exist").exists()
            client.send({"method": "session/cancel", "params": {"sessionId": session}})
            assert client.result(pending)["stopReason"] == "cancelled"
            option = next(o for o in permission["params"]["options"] if o["kind"] == "allow_once")
            client.send({"id": permission["id"], "result": {"outcome": {
                "outcome": "selected", "optionId": option["optionId"],
            }}})
            # A subsequent RPC is a transport barrier after the stale callback.
            client.rpc("session/set_mode", {"sessionId": session, "modeId": "full-access"})
        assert not (root / "must-not-exist").exists(), "A stale approval executed canceled work"
        server.shutdown()
    print(json.dumps({"version": version, "fresh": True, "resume": True,
                      "config": True, "native_tool_rounds": 2,
                      "cancel_pending_permission": True, "late_approval_ignored": True}))


if __name__ == "__main__":
    main()
