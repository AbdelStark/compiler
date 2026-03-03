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
- `[COMPLETED]` R7: Add crypto runtime tests (`tests/runtime_crypto_test.rs`) for real `OP_CHECKSIG`, `OP_CHECKSIGFROMSTACK`, and failure paths.
- `[COMPLETED]` R8: Implement real secp256k1 crypto verification (no mock checksig path) in `src/runtime/env.rs`.
- `[COMPLETED]` R9: Add artifact integration tests for `examples/htlc.json` runtime execution path.
- `[COMPLETED]` R10: Implement artifact loader + function/variant selector for runtime execution.
- `[COMPLETED]` R11: Extend CLI with `compile`, `run`, and `debug` subcommands in `src/main.rs` (with legacy compile mode compatibility).
- `[COMPLETED]` R12: Add telemetry instrumentation (`tracing`) with per-opcode stack snapshots.
- `[COMPLETED]` R13: Build TUI debugger scaffold with `ratatui`/`crossterm` panes and controls.
- `[COMPLETED]` R14: Implement full opcode surface from `src/opcodes/mod.rs`, including introspection/group/asset opcodes and conversion/EC operations.
- `[COMPLETED]` R15: Final validation gates (`cargo test`, `cargo fmt --check`) plus runtime-specific matrix suites.
- `[COMPLETED]` R16: Add high-coverage runtime suites:
  - `tests/runtime_extended_opcodes_test.rs`
  - `tests/runtime_introspection_test.rs`
  - `tests/runtime_examples_e2e_test.rs`
  - `tests/runtime_cli_matrix_test.rs`
  - `tests/runtime_cli_run_test.rs` trace coverage
- `[COMPLETED]` R17: Add opcode-surface completeness test (`tests/runtime_opcode_surface_test.rs`) that asserts every opcode declared in `src/opcodes/mod.rs` is handled by dispatcher (no `UnknownOpcode`/`UnsupportedOpcode`).
- `[COMPLETED]` R18: Add deterministic replay and failure-classification vectors:
  - `tests/runtime_determinism_test.rs`
  - `tests/runtime_failure_classification_test.rs`
- `[COMPLETED]` R19: Debugger depth upgrade with breakpoint support:
  - CLI: `arkadec debug ... --breakpoint <ip>`
  - TUI: toggle breakpoint key `b`, continue-to-breakpoint semantics, breakpoint markers in Script pane
  - validation in `tests/runtime_cli_debug_boot_test.rs`
- `[COMPLETED]` R20: ABI-aware default binding inference:
  - `LoadedProgram` now carries parameter type metadata from artifact ABI.
  - Runtime default bindings use ABI types (`pubkey`, `signature`, `bytes*`, `int`, `bool`) before heuristics.
  - validation in `tests/runtime_binding_inference_test.rs`.
- `[COMPLETED]` R21: External semantic parity audit harness:
  - Added executable parity vector corpus in `tests/parity_vectors/*.json`.
  - Added local parity verifier test `tests/runtime_parity_vectors_test.rs`.
  - Added external bridge test `tests/runtime_external_parity_audit_test.rs` via `ARKADE_PARITY_EXTERNAL_CMD`.
  - Added protocol documentation `.harness/PARITY_AUDIT.md`.
- `[COMPLETED]` R22: External adapter + deep parity equivalence:
  - Added in-repo external adapter binary `src/bin/arkade_parity_adapter.rs`.
  - External bridge now validates full parity contract: outcome class, error code, final stacks, and telemetry snapshots.
  - Bridge auto-resolves in-repo adapter via `CARGO_BIN_EXE_arkade_parity_adapter` when `ARKADE_PARITY_EXTERNAL_CMD` is not set.

## Active Focus
Current target: closed.
Next transition: cross-runtime adapter integration against real `arkd`/`introspector` executable for non-self parity.

## Open Risks
- Introspection and streaming-hash semantics are implemented concretely for in-process runtime context, but still need parity coverage against real `arkd`/`introspector` adapter outputs (current fallback adapter uses this repo runtime implementation in a separate process).
- Parity vectors currently cover targeted behavior classes; additional edge vectors are still needed for exhaustive opcode path coverage.

## Validation Log
- `[COMPLETED]` `cargo fmt --check` passed.
- `[COMPLETED]` `cargo test` passed (including new runtime and CLI tests).
- `[COMPLETED]` `cargo test --test runtime_crypto_test --test runtime_extended_opcodes_test --test runtime_introspection_test --test runtime_examples_e2e_test` passed.
- `[COMPLETED]` `cargo test --test runtime_cli_run_test --test runtime_cli_debug_boot_test --test runtime_cli_matrix_test` passed.
- `[COMPLETED]` `cargo test --test runtime_opcode_surface_test --test runtime_determinism_test --test runtime_failure_classification_test --test runtime_cli_debug_boot_test` passed.
- `[COMPLETED]` full `cargo test` passed after breakpoint/debugger and parity-hardening additions.
- `[COMPLETED]` full `cargo test` passed after ABI-aware binding inference addition.
- `[COMPLETED]` `cargo test --test runtime_parity_vectors_test --test runtime_external_parity_audit_test` passed.
- `[COMPLETED]` `cargo test --test runtime_external_parity_audit_test` passed with in-repo external adapter path fallback.
