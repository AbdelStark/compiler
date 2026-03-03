# Runtime Validation Matrix

## Validation Objective
Prove the in-process VM is deterministic, panic-free for script input, and behaviorally aligned with Arkade artifact semantics.

## Soundness Model
- Determinism: same `(artifact, function, variant, env)` always yields identical `(outcome, final stack, trace)`.
- Safety: no `panic!` during opcode execution on invalid scripts; failures are explicit `RuntimeError`.
- Total classification: every run ends in exactly one class:
  - `ScriptTrue`
  - `ScriptFalse`
  - `RuntimeError`

## Mathematical Invariants
1. Stack depth invariant:
   - For each opcode `op`, `depth_after = depth_before - pops(op) + pushes(op)`.
   - Enforce `depth_before >= pops(op)` else `RuntimeError::StackUnderflow`.
2. Instruction pointer invariant:
   - `ip` is monotonic within a linear segment.
   - Branch ops (`OP_IF` / `OP_ELSE` / `OP_ENDIF`) move to matching structural boundary only.
3. Halt invariant:
   - `halted == true` implies no further state mutation.
   - Final outcome is immutable once set.
4. Failure separation invariant:
   - `OP_VERIFY(false)` => `ScriptFalse`.
   - Decoder/stack/unsupported errors => `RuntimeError`.

## Programmatic TDD Strategy

## Unit tests (opcode and primitives)
- `tests/runtime_stack_test.rs`
  - push/pop/dup/drop/nip behavior
  - underflow paths
- `tests/runtime_arithmetic_test.rs`
  - add/sub/mul/div success
  - division by zero and invalid numeric encoding
- `tests/runtime_crypto_test.rs`
  - `OP_SHA256`, `OP_EQUALVERIFY`
  - `OP_CHECKSIG` and `OP_CHECKSIGVERIFY` via mock provider
- `tests/runtime_control_flow_test.rs`
  - `OP_IF`/`OP_ELSE`/`OP_ENDIF` path correctness
- `tests/runtime_error_test.rs`
  - unknown opcode
  - unsupported opcode classification

## Integration tests (full artifact execution)
- `tests/runtime_artifact_htlc_test.rs`
  - load `examples/htlc.json` and run `claim` and `refund` variants
- `tests/runtime_artifact_single_sig_test.rs`
  - end-to-end pass/fail checks for both variants
- `tests/runtime_cli_run_test.rs`
  - `arkadec run` exit codes (`0`, `1`, `2`)
  - trace output includes per-opcode state transitions
- `tests/runtime_cli_debug_boot_test.rs`
  - `arkadec debug` initializes TUI state and panes (non-interactive smoke mode)

## Telemetry Validation
- Every executed opcode must emit one step event containing:
  - `ip`
  - opcode/push token
  - stack depth before/after
  - result classification
- Integration tests assert at least one trace event per instruction.

## Regression / Coverage Gates
- Targeted feature gate during development:
  - `cargo test --test runtime_stack_test`
  - `cargo test --test runtime_arithmetic_test`
  - `cargo test --test runtime_artifact_htlc_test`
- Full gate before completion:
  - `cargo test`
  - `cargo fmt --check`

## Non-Goals for Initial Runtime Cut
- Real signature cryptography verification.
- Full introspection opcode semantics beyond minimal artifact compatibility.
- Remote execution or networking.

## Risk Register
- Placeholder resolution ambiguity (`<name>` literals vs bound values).
  - Mitigation: strict-mode toggle + explicit tests for both modes.
- Existing opcode set is larger than initial runtime milestone.
  - Mitigation: `UnsupportedOpcode` with clear telemetry and incremental coverage plan.
- TUI flakiness in CI terminals.
  - Mitigation: isolate rendering model and test with headless smoke mode.

