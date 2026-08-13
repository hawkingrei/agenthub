---
sidebar_position: 3
---

# Notifications

AgentHub shows completion state in the web UI and can deliver an encrypted Web
Push notification when an agent process finishes.

## Browser Push Requirements

- A browser with service worker, Notification, and Push API support.
- HTTPS outside localhost.
- A signed-in AgentHub session with permission to create a subscription.
- A configured VAPID subject.

The frontend registers `/sw.js`. After sign-in, grant the browser notification
permission so it can submit the browser subscription to AgentHub.

## Configuration

```toml
[push]
subject = "mailto:ops@example.com"
keys_path = "~/.agenthub/vapid.json"
```

The VAPID key pair is created at `keys_path` on first startup and reused across
restarts. Protect and back up this file alongside other AgentHub state.

Root operators can inspect or rotate keys from **Admin**. The equivalent API is:

```bash
curl --fail -X POST \
  -H "Authorization: Bearer ${AGENTHUB_TOKEN}" \
  http://127.0.0.1:8080/api/push/vapid_rotate
```

Rotation invalidates the public key associated with existing browser
subscriptions. Use it for an intentional security event, then have users sign
in and subscribe again; do not rotate it as routine cleanup.

## What Is Sent

The current completion payload contains the event type, agent ID, session ID,
and server timestamp:

```json
{
  "type": "agent_completed",
  "agent_id": "agent-id",
  "session_id": "session-id",
  "ts": 1710000000
}
```

Push delivery is best effort. The session history and current Agent status are
the authoritative records.

## Validate Push

1. Sign in with a browser profile that permits notifications.
2. Start a short agent run.
3. Move the tab to the background.
4. Let the agent process exit.
5. Confirm the system notification and the persisted AgentHub timeline.

## Troubleshooting

| Symptom | Check |
|---------|-------|
| No permission prompt | Reset the site's notification permission and reload. |
| Subscription request is `401` or `403` | Sign in again and confirm the account has push-subscribe capability. |
| Push works on localhost but not a hostname | Use HTTPS and confirm the service worker is registered for the same origin. |
| Delivery stops after key rotation | Re-subscribe every browser profile. |
| Server warns that push is disabled | Set a valid `push.subject` and ensure `keys_path` is writable. |

AgentHub does not currently document webhook, email, Slack, or Discord delivery
as supported notification channels.
