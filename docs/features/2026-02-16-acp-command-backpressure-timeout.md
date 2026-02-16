# ACP Command Backpressure Timeout

## Background

ACP commands (`prompt`, `set_mode`, `set_model`, `set_config`, `cancel`) are
forwarded through a bounded `mpsc` queue into the ACP worker loop. Without a
send timeout, callers can wait indefinitely if the queue remains full due to
worker-side stalls or long-running command processing.

## Scope

- Keep the ACP command queue bounded.
- Add a timeout guard on command enqueue (`AcpHandle::send`).
- Return explicit errors on timeout vs. channel-closed conditions.
- Add unit tests for timeout and closed-channel branches.

## Key Decisions

- Keep queue capacity at `64` (`ACP_COMMAND_CHANNEL_CAPACITY`).
- Add `ACP_COMMAND_SEND_TIMEOUT = 5s`.
- Use `tokio::time::timeout(...)` around `mpsc::Sender::send(...)`.
- Emit a warning log when timeout occurs and include session ID + timeout ms.
- Preserve existing behavior for closed channel with an explicit
  `acp command channel closed` error.

## Validation

```bash
cargo test -p agenthub-acp
cargo test --all
```

Expected outcomes:

- command enqueue no longer waits forever under sustained backpressure;
- timeout path returns a clear error and logs a warning;
- closed receiver path still returns deterministic channel-closed error.
