---
sidebar_position: 3
---

# Notifications

AgentHub can notify you when a task is completed, both in-app and via browser push notifications.

## In-App Notification

By default, completion signals appear in the AgentHub UI:

- A notification badge on the agent card
- Visual indicators in the session view
- History entries showing completion state

## Browser Push Notification

AgentHub supports Web Push API notifications that work even when the browser tab is not focused or closed.

### How It Works

1. **VAPID Keys**: AgentHub uses VAPID (Voluntary Application Server Identification) for secure push notifications
2. **Subscription**: Each browser creates a unique subscription tied to your user account
3. **Delivery**: When an agent completes, the backend sends a push notification via your browser's push service

### Configuration

VAPID keys are auto-generated on first startup and stored in `~/.agenthub/vapid_keys.json`:

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

1. **Grant Permission**: Click the notification bell icon in the AgentHub UI
2. **Browser Prompt**: Allow notifications when the browser asks
3. **Verify Subscription**: Check the settings page shows "Push enabled"

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
   - System notification appears (if push enabled)
   - In-app notification badge updates
   - History shows completed state

## Troubleshooting

### Push Notifications Not Received

| Symptom | Check | Solution |
|---------|-------|----------|
| No browser prompt | Permission already denied | Reset site permissions in browser settings |
| Subscription fails | VAPID keys missing | Restart AgentHub to auto-generate keys |
| Silent failures | Check browser console for errors | Verify `vapid_subject` is configured |
| Works locally but not remote | HTTPS required | Push API requires secure context (HTTPS) |

### Configuration Options

Add to `~/.agenthub/config.toml`:

```toml
[vapid]
# Required for push notifications
subject = "mailto:admin@example.com"

# Optional: custom keys path (default: ~/.agenthub/vapid_keys.json)
keys_path = "/custom/path/vapid_keys.json"
```

The `vapid.subject` should be a contact email or URL for your application.

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
