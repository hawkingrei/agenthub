# Linkerdog Core

Core runtime/session engine for Linkerdog ACP.

This crate owns:

- ACP Agent implementation
- session persistence (`.cache/context/run/<session_id>/`)
- permission + tool-call baseline flow
- runtime config parsing (`provider`, `model`, `mode` overrides)
