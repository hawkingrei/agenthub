---
sidebar_position: 3
---

# Login and Access

## Initialize the First Root Operator

On a new AgentHub instance, the login page shows **First-run setup**. Create the
first root operator there:

1. Enter a username. Usernames cannot contain `@` because mentions reserve it.
2. Enter a display name.
3. Set a password.
4. Select **Initialize Root**.

This first account can manage instance-wide settings, users, and remote Agent
Nodes. Initialize it only from a trusted network and store its credentials in
your normal secrets manager.

## Regular Login

After initialization, sign in with the username and password created for this
instance. A successful login creates a 12-hour bearer session in the browser.

If passkeys are enabled, AgentHub may ask you to finish a WebAuthn challenge or
register a credential after the password step. Configure the public HTTPS
origin before enabling passkeys outside localhost.

## Joining an Existing Teamspace

An invitation link creates an operator account for the invited Teamspace. Do
not use the invite flow to initialize the root account, and do not reuse an
invite from a different AgentHub instance.

## If Login Fails

1. Open `/api/auth/status` and confirm the instance reports
   `root_initialized: true`.
2. Confirm the username belongs to this instance and the server clock is
   correct.
3. If passkey verification fails, confirm the configured `web.rp_id` and
   `web.rp_origin` match the URL in the browser.
4. Check server logs for the corresponding authentication audit event.

Do not delete the database or VAPID files to recover an account. Restore a
known-good backup or use an authenticated administrative flow instead.
