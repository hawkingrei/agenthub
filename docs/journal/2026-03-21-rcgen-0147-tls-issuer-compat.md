# rcgen 0.14.7 TLS Issuer Compatibility

## Summary

- updated internal gRPC TLS material generation to match the `rcgen 0.14.7` signing API

## Details

- `generate_tls_material(...)` now builds an `Issuer` from the CA certificate parameters and CA key
- server and bootstrap client certificates are signed through that `Issuer`
- this keeps the existing certificate topology unchanged while removing the old three-argument
  `signed_by(...)` calls that no longer compile on `rcgen 0.14.7`

## Validation

- `cargo test ensure_tls_material_generates_files -- --nocapture`
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
