---
active: true
iteration: 3
max_iterations: 200
completion_promise: "ALL_WARNINGS_FIXED"
started_at: "2026-01-08T20:29:57Z"
---

Fix ALL Rust compiler warnings. Run 'cargo check 2>&1 | head -100'. STRICT RULES: DELETE unused variables, imports, functions, structs, and dead code entirely. NEVER use these shortcuts: #[allow(...)], underscore prefixes (_var), let _ =, commenting out code, todo\!(), unimplemented\!(), fake println/dbg to use variables, adding pub to avoid warnings, drop() to fake-use variables, #[cfg(test)] to hide code, if false blocks. If removing code breaks something, fix the actual dependency. Actually clean the codebase. Run 'cargo check' then 'cargo clippy -- -D warnings'. Every 10 iterations run 'cargo clean' to free disk space. When BOTH check and clippy show ZERO warnings, output: <promise>ALL_WARNINGS_FIXED</promise>
