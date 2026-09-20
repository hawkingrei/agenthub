pub(super) const SCRIPT: &str = r#"import json
import pathlib
import subprocess

root = pathlib.Path(__file__).parent
control = json.loads((root / "native-settings.json").read_text())["control"]


def actor(*args):
    result = subprocess.run(
        [control, "actor", *args, "--json"], capture_output=True, text=True
    )
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


page = actor("loop-context", "--limit", "1")
activation = page["activation"]
sources = page["sources"]
while page["next_cursor"] is not None:
    page = actor("loop-context", "--limit", "1", "--after-source-id", page["next_cursor"])
    sources.extend(page["sources"])
details = [actor("loop-source", "--source-id", source["id"]) for source in sources]
child = subprocess.run(
    [control, "actor", "team-tasks", "--actor-id", "native-child", "--json"],
    capture_output=True,
    text=True,
)
assert child.returncode != 0

if activation["actor_id"] == "planner" and not any(
    detail["mailbox_message"] for detail in details
):
    actor(
        "team-task-create", "--title", "Native offline review",
        "--priority", "medium", "--assigned-member-id", "worker",
    )
    actor("send", "--to", "worker", "--text", "dispatch evidence", "--idempotency-key", "native-dispatch")
    stage = "dispatch"
elif activation["actor_id"] == "worker":
    assert any(
        detail["mailbox_message"]
        and detail["mailbox_message"]["payload"]["text"] == "dispatch evidence"
        for detail in details
    )
    task_ids = {
        detail["source"]["input"]["references"]["task_id"]
        for detail in details
        if detail["source"]["input"]["references"]["task_id"]
    }
    assert len(task_ids) == 1
    task_id = task_ids.pop()
    assert actor("team-task-show", "--task-id", task_id)["task"]["assigned_member_id"] == "worker"
    denied = subprocess.run(
        [control, "actor", "team-task-update", "--team-id", activation["team_id"],
         "--task-id", task_id, "--status", "completed", "--note-kind", "result",
         "--note", "Self acceptance is forbidden", "--json"],
        capture_output=True, text=True,
    )
    assert denied.returncode != 0 and "coordinator" in denied.stderr.lower(), denied.stderr
    assert actor("team-task-show", "--task-id", task_id)["task"]["status"] == "open"
    actor("team-task-note", "--task-id", task_id, "--kind", "result", "--text", "Native evidence is ready")
    actor("send", "--to", "planner", "--text", "worker report", "--idempotency-key", "native-report")
    stage = "report"
else:
    assert any(
        detail["mailbox_message"]
        and detail["mailbox_message"]["payload"]["text"] == "worker report"
        for detail in details
    )
    task = next(task for task in actor("team-tasks") if task["title"] == "Native offline review")
    detail = actor("team-task-show", "--task-id", task["id"])
    assert any(
        note["from_actor_id"] == "worker" and note["text"] == "Native evidence is ready"
        for note in detail["notes"]
    )
    actor(
        "team-task-update", "--team-id", activation["team_id"], "--task-id", task["id"],
        "--status", "completed", "--note-kind", "decision", "--note", "Accepted native evidence",
    )
    stage = "accept"

with (root / "native-cycle.jsonl").open("a") as log:
    log.write(json.dumps({"stage": stage, "activation": activation["id"], "child_identity_denied": True}) + "\n")
outcome = root / "native-outcome.json"
outcome.write_text(json.dumps({"kind": "handoff"}))
actor("loop-finish", "--outcome-file", str(outcome))
"#;
