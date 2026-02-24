# Database Init Index Logging

## Background

Index creation failures were silently ignored during database initialization. If an index fails to create (permissions, corruption, pragma restrictions), the app may run with degraded query performance and no signal in logs.

## Scope

- Log warnings when index creation fails during `init_db`.
- Log unexpected failures when adding the `auth_sessions.revoked_at` column.

## Key Decisions

- Keep initialization resilient (no hard failure) but emit `warn` logs to surface operational issues.
- Suppress the expected "duplicate column name" error for the optional `ALTER TABLE` migration.

## Validation

```bash
Start the service and confirm warning logs appear if index creation fails.
```
