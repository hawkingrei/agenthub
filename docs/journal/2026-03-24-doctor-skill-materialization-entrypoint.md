# Doctor Skill Materialization Entrypoint

## Summary

- added a real top-level `agenthub doctor` entrypoint
- wired `doctor` before normal server startup so it materializes managed skills
  and exits instead of falling through to the HTTP server path
- added focused unit coverage for `doctor` argument parsing and output reporting

## Why

- `agenthub doctor` was intended to initialize managed runtime skills
- the binary only recognized `agenthub actor ...`, so `agenthub doctor` fell
  through to the normal application startup path and looked like it "hung"
  while actually booting the server

## Validation

- `cargo test -p agenthub doctor_cli -- --nocapture`
- `git diff --check`
- `HOME=/tmp/agenthub-doctor-e2e-$RANDOM target/debug/agenthub doctor`
