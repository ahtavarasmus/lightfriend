# parent-proxy

Lightweight Rust proxy for routing webhooks between Nitro Enclaves based on user migration status.

## Development

```bash
cargo build           # Build
cargo test            # Run all tests
cargo run             # Run locally (uses HTTP instead of VSOCK)
```

## Test Protection

Tests in `tests/` folder are protected from accidental edits. They serve as a safety net to catch unintended behavior changes.

- **Editing existing tests** requires explicit user approval
- **Adding new tests** is always allowed

If blocked when editing a test, ask the user for permission first.

## Routing Logic

```
active_enclave value -> Enclave Target
------------------------------------
"old"                -> OLD enclave
NULL                 -> NEW enclave
"new"                -> NEW enclave
User not found       -> NEW enclave
```
