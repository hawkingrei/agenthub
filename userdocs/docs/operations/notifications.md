---
sidebar_position: 3
---

# Notifications

AgentHub can notify you when a task is completed, both in-app and via browser push notifications.

## In-App Notification

By default, completion signals appear in the AgentHub UI:

- Runtime and connection badges in the active workspace
- Structured completion state in ACP / Team timelines
- Browser notifications when push is enabled

## Browser Push Notification

AgentHub supports Web Push API notifications that work even when the browser
tab is not focused or closed.

### How It Works

1. **Service worker**: the frontend registers `/sw.js` automatically
2. **Subscription**: after successful login or join, AgentHub attempts to attach a Push subscription for the current browser session
3. **Delivery**: when an agent completes, the backend sends a push notification via your browser's push service

### Configuration

VAPID keys are auto-generated on first startup and stored in `~/.agenthub/vapid.json`:

```json
{
  "public_key": "<base64-encoded-public-key>",
  "private_key": "<base64-encoded-private-key>"
}
```

To rotate VAPID keys (invalidates existing subscriptions):

```bash
# Via API (requires admin)
curl -X POST -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/admin/push/rotate-keys
```

Or use the Admin Console UI to rotate keys.

### Enabling Push Notifications

1. Open AgentHub in a browser that supports service workers and Push API
2. Sign in or complete the join/bootstrap flow
3. Allow notifications when the browser asks
4. Keep using the same browser profile for that subscription

### Notification Payload

Push notifications include:

```json
{
  "type": "agent_completed",
  "agent_id": "agent-uuid",
  "session_id": "session-uuid",
  "ts": 1710000000
}
```

## Quick Validation

1. Start a short task
2. Switch browser tab or minimize the window
3. Wait for task completion
4. Confirm:
   - System notification appears (if push is enabled)
   - The active AgentHub workspace reflects the completed state
   - Timeline / history shows the completion event

## Troubleshooting

### Push Notifications Not Received

| Symptom | Check | Solution |
|---------|-------|----------|
| No browser prompt | Permission already denied or browser does not support Push | Reset site permissions, then refresh and sign in again |
| Subscription fails | VAPID keys missing | Restart AgentHub to auto-generate keys |
| Silent failures | Check browser console for errors | Verify `push.subject` is configured in the `[push]` section |
| Works locally but not remote | HTTPS required | Push API requires secure context (HTTPS) |

## Installable App Behavior

AgentHub is installable as a standalone browser app on supported platforms.

- Installability and push both rely on the same service worker registration
- The installed app still prefers fresh server-delivered HTML and hashed assets
- Do not treat the installed experience as an offline-first shell

### Configuration Options

Add to `~/.agenthub/config.toml`:

```toml
[push]
# Required for push notifications
subject = "mailto:admin@example.com"

# Optional: custom keys path (default: ~/.agenthub/vapid.json)
keys_path = "/custom/path/vapid.json"
```

The `push.subject` value should be a contact email or URL for your application.

## Security Considerations

- VAPID private keys are sensitive — protect the keys file
- Subscriptions are per-user; other users won't receive your notifications
- Push services (FCM, APNS, etc.) only receive encrypted payload headers

## Future Extensions

Planned notification channels:

- **Webhook**: POST callbacks to external systems
- **Email**: SMTP-based notifications for critical completions
- **Slack/Discord**: Direct integrations with team chat

## Recommended Setup

- Enable push notifications for long-running tasks (>5 minutes)
- Use clear agent naming conventions for easy notification identification
- Combine notifications with session history for efficient follow-up
- Rotate VAPID keys periodically if you have high security requirements
