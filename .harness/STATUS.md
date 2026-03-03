# Arkade Runtime Status

Updated: 2026-03-03

## State Legend
- `[TODO]`: scoped but not started
- `[IN_PROGRESS]`: active implementation/testing step
- `[VALIDATING]`: implementation complete, running validation gates
- `[COMPLETED]`: accepted and closed

## State Machine
`PENDING -> ASSIGNED -> IN_PROGRESS -> REVIEW -> APPROVED -> DONE`

This board maps to the required tags above by execution phase:
- `PENDING/ASSIGNED` => `[TODO]`
- `IN_PROGRESS` => `[IN_PROGRESS]`
- `REVIEW/APPROVED` => `[VALIDATING]`
- `DONE` => `[COMPLETED]`

## Work Board
- `[COMPLETED]` H0: Initialize `.harness/` with `SPECS.feature`, `PLAN.md`, `VALIDATION.md`, `STATUS.md`.
- `[COMPLETED]` R1: Write first failing unit test `tests/runtime_stack_test.rs` for push + `OP_DUP`/`OP_DROP`/`OP_NIP` happy path and underflow behavior.
- `[COMPLETED]` R2: Implement `src/runtime/value.rs`, `src/runtime/error.rs`, `src/runtime/stack.rs` to satisfy R1.
- `[COMPLETED]` R3: Add `VMState` step loop in `src/runtime/vm.rs` with halt/result separation (`ScriptFalse` vs `RuntimeError`).
- `[COMPLETED]` R4: Add `OpcodeDispatcher` in `src/runtime/dispatcher.rs` and wire stack opcodes.
- `[COMPLETED]` R5: Add arithmetic tests (`tests/runtime_arithmetic_test.rs`) for `OP_ADD64`, `OP_SUB64`, `OP_MUL64`, `OP_DIV64`.
- `[COMPLETED]` R6: Implement arithmetic/comparison handlers and numeric decoding safety.
- `[COMPLETED]` R7: Add crypto tests (`tests/runtime_crypto_test.rs`) for `OP_SHA256`, `OP_EQUALVERIFY`, `OP_CHECKSIG` (mocked).
- `[COMPLETED]` R8: Implement crypto handlers and checksig provider trait in `src/runtime/env.rs`.
- `[COMPLETED]` R9: Add artifact integration tests for `examples/htlc.json` runtime execution path.
- `[COMPLETED]` R10: Implement artifact loader + function/variant selector for runtime execution.
- `[COMPLETED]` R11: Extend CLI with `compile`, `run`, and `debug` subcommands in `src/main.rs` (with legacy compile mode compatibility).
- `[COMPLETED]` R12: Add telemetry instrumentation (`tracing`) with per-opcode stack snapshots.
- `[COMPLETED]` R13: Build TUI debugger scaffold with `ratatui`/`crossterm` panes and controls.
- `[COMPLETED]` R14: Add unsupported opcode handling matrix for remaining opcodes used in examples.
- `[COMPLETED]` R15: Final validation gates (`cargo test`, `cargo fmt --check`).

## Active Focus
Current target: closed.
Next transition: runtime milestone accepted and ready for incremental opcode expansion.

## Open Risks
- Signature verification remains mocked in first cut; real cryptographic semantics are deferred.
- Some introspection opcodes may stay unsupported initially and must return explicit runtime errors.

## Validation Log
- `[COMPLETED]` `cargo fmt --check` passed.
- `[COMPLETED]` `cargo test` passed (including new runtime and CLI tests).
