# RFC 001: Advanced Runtime Features

**Status**: Draft
**Date**: 2026-03-03
**Owner**: compiler runtime improvement track

## Goal

Make the Arkade runtime stack truly production-grade for end-to-end Ark contract testing and execution by improving correctness, determinism, diagnostics, security boundaries, and developer ergonomics, while preserving current compiler output compatibility.

## Problem Statement

The current runtime is useful for dry-runs and happy-path examples, but it is not yet an end-to-end execution platform suitable for realistic contract debugging and CI-grade validation.

The codebase already has a strong foundation:

- A broad opcode surface in `src/dispatcher.rs` and `src/opcodes/mod.rs`.
- Good parity hooks (`tests/runtime_external_introspector_*`, external adapter path).
- A working CLI (`compile`, `run`, `debug`) and a browser playground path with WASM execution.

However, runtime behavior still depends on defaults and assumptions that limit confidence in real-world transaction testing.

## Reflection on Current State

This section is intentionally self-critical and specific to what exists now:

### What works well

- Deterministic enough for many local examples; both compiler and runtime can execute existing sample contracts.
- Good baseline failure classification (`verify_false` vs `runtime_error`) with a small `RuntimeErrorCode` taxonomy.
- Existing parity and deterministic test scaffolding.
- Playable TUI debugger with breakpoints.

### What is currently missing / weak

#### 1) Execution fidelity

- Runtime execution context is synthetic by default.
  - `ExecutionEnv`/`TxContext` starts with randomized synthetic fixture (`TxContext::sample()`), not real tx inputs/outputs.
  - `arkadec run` and WASM execution do not require a real transaction or deterministic fixture.
- Signature verification semantics are implementation-local and incomplete for parity with Bitcoin-like signing realities:
  - `sign_message_for_label` is deterministic key-label-derived convenience, not transaction-aware signer context.
  - `OP_CHECKSIG`/`OP_CHECKSIGFROMSTACK` paths use simplified message handling.
- Several conversions are permissive:
  - `StackValue::as_bool()` treats any non-zero/non-empty bytes as true.
  - Symbol coercions can create surprising behavior when strict validation is expected.
- Resource governance is minimal:
  - stack size has only static max-depth guards.
  - no step/cost limits, no CPU budget, no opcode budget, no memory budget by tx size.

#### 2) Compiler + runtime semantic gaps

- Parser/compile flow intentionally leaves known TODOs:
  - Variable reassignment is parsed and accepted in grammar but not compiled meaningfully.
  - Let bindings are not tracked as typed runtime variables.
  - Array indexing and array length are stubs.
  - Function call statements are parsed but effectively ignored.
- `for` unrolling over unknown iterables is best-effort fallback, not strongly validated.
- Default binding generation can silently paper over missing data and may emit behavior that does not match production usage.

#### 3) Observability and debuggability

- Telemetry is stack snapshots and status, but lacks:
  - source mapping (line/column or source span)
  - opcode timing, step counts by opcode family, or resource usage metrics
  - structured machine-readable output on CLI for CI parsing.
- Debugger output is interactive-only and local to terminal TUI.

#### 4) API and developer UX

- CLI `run` accepts only source/artifact + function + variant + bindings; cannot run against explicit tx context or fixture inputs.
- Binding format in both CLI and playground is manual and not standardized with schema validation.
- Runtime artifacts and execution output are not always reproducibility-friendly (no canonical normalized trace schema across CLI, WASM, and parity adapter).
- Playground runtime is educational and useful but no persisted environment presets, no tx fixtures, and no shareable execution profile.

#### 5) Security/ops concerns

- No explicit sandbox hardening around payload size, binding complexity, opcode blowup, or attacker-controlled large scripts.
- No explicit feature gates/feature flags for unsafe opchains.
- Error provenance is useful, but not yet sufficient for auditing/forensics without additional context.

## Proposal

Introduce **Advanced Runtime** with three design principles:

1. **Parity-first behavior**: deterministic execution that can be trusted with real artifacts and tx fixtures.
2. **DX by default**: standard inputs/outputs, explicit validation, and reproducible diagnostics.
3. **Production-grade operations**: budgets, policy checks, and clear compatibility behavior.

### 1. Deterministic Transaction-Context Execution

#### 1.1 Canonical fixture schema

Create a schema-backed runtime input payload (JSON/serde):

- Contract (compiled JSON)
- Function selector + variant
- Bindings
- Tx context:
  - txid, version, locktime, weight
  - inputs/outputs with value, sequence, scriptPubKey, outpoint, issuance
  - assets with txid/gidx/amount/control/metadata data
  - asset groups with sums and metadata

#### 1.2 CLI + WASM contract

- Add explicit context input flags / JSON argument for `arkadec run`:
  - `--context-file`
  - `--context-json`
  - `--context-strict` (validation and unknown-field policy)
- Extend WASM API to accept optional `context_json` for `execute_contract_json` and `execute_source`.
- Keep legacy behavior (synthetic context) as fallback for convenience.

### 2. Runtime Semantics + Validation Hardening

#### 2.1 Strictness modes

- Keep current permissive default for local exploration, add explicit strict modes:
  - `strict_placeholders` (already present)
  - `strict_types`: disallow loose coercion in `as_bool`, `as_i64` for mismatched input
  - `strict_bindings`: require complete schema validation before execution

#### 2.2 Deterministic signature model

- Introduce explicit signer contexts in runtime execution context:
  - optional tx-level message template id
  - deterministic signer lookup by label/alias + message policy
- Make signature failures fully auditable in telemetry.

#### 2.3 Runtime policy hooks

- Add pluggable checks before execution:
  - max steps
  - max script length
  - max stack/alt stack depth and per-step growth limits
  - op budget (future fee-like semantics)

### 3. Compiler/Runtimes Semantics Completion

#### 3.1 Compiler TODO closure

- Implement `VarAssign` lowering/slot model.
- Implement array features declared by grammar (`array_index_access`, `array_literal` support) consistently.
- Compile function-call statements into explicit runtime-call behavior or reject early with precise parser errors.

#### 3.2 Type-aware binding inference

- Improve `default_bindings_for_program`:
  - infer defaults from concrete parameter type list and placeholder usage.
  - never silently coerce unknown placeholders into arbitrary numeric defaults without warning.

#### 3.3 Introspection parity

- Keep parity with existing opcodes but add:
  - richer property/value decoding in env defaults
  - explicit missing-index and missing-asset diagnostics with structured fields

### 4. Next-level Observability

#### 4.1 Structured trace format v1

- Add normalized trace event schema with:
  - ip, token, status
  - stack_before/stack_after
  - source span (file+line+col if available)
  - step_id, elapsed_nanos (optional)
  - policy counters (steps, conditional jumps, branch path)

#### 4.2 Deterministic replay

- Add `trace_id` and optional `seed` and `runtime_options` in execution output.
- Enable local replay by storing:
  - contract + context + bindings + options
  - trace version.

### 5. CLI and Playground Modernization

#### 5.1 `run` command as execution protocol

- Output formats:
  - plain text (current)
  - JSON (`--output json`), CSV metrics (`--output metrics`) for CI.
- Add `--list-functions`, `--dump-default-context`, and `--profile` for speed diagnostics.
- Add `--bind-file` alongside repeated `--bind` flags.

#### 5.2 Playground upgrades

- Add tx-context editor/preset import for runtime tab.
- Add trace playback with source-linked stepping.
- Add “contract matrix runner” (run all functions/variants).
- Add “fixture import” and fixture sharing URL mode.

### 6. Safety and Production Hardening

- Add hard fail for:
  - malformed bindings (explicit diagnostics)
  - unknown placeholders when strict mode enabled
  - opcodes outside an allowed set in configured policy mode
- Add execution mode presets:
  - `development`, `simulation`, `ci`, `safety`.

## Concrete Milestones

### Milestone A: Deterministic Execution Baseline (2–4 weeks)

- Add context schema + runtime context deserialization.
- Wire CLI/WASM execution to accept context inputs.
- Keep synthetic context fallback.
- Add CLI JSON output mode and trace-version metadata.

### Milestone B: Semantic Completion + Strictness (4–6 weeks)

- Implement compiler TODOs:
  - var reassignment
  - array index/length
  - function-call handling/explicit rejection.
- Add strict conversion behavior and runtime policy guardrails.
- Add schema validation with clear errors.

### Milestone C: DX/Observability + Production Features (6–8 weeks)

- Structured source-linked trace + deterministic replay bundle.
- Playground and debugger improvements.
- CLI execution matrix and fixture-driven smoke test mode.

### Milestone D: Release Candidate (2–3 weeks)

- Final parity regression and external adapter parity extension.
- Security hardening and performance micro-benchmarks.
- Documentation updates and migration guide.

## Acceptance Criteria

1. Deterministic parity test can run same contract against same context across:
   - CLI
   - WASM
   - external adapter parity test harness.
2. Runtime executes against explicit fixture without synthetic assumptions.
3. `strict_*` modes are opt-in and backward compatible.
4. Existing examples and default `cargo test` remain green.
5. At least one contract-level matrix mode validates all function variants end-to-end.
6. Trace format is stable and documented with version and schema.

## Non-goals (initial phase)

- Direct consensus-layer embedding or C++ interpreter replacement.
- Full Bitcoin Script equivalence for script-level minutiae not already modeled by this repo.
- Redesigning the parser into another language.

## Risks and Mitigations

### Risk: Breaking existing scripts

- Mitigation: default compatibility mode remains default; strictness and policy features are opt-in.

### Risk: Performance regressions from richer checks

- Mitigation: feature-gated checks, baseline-only mode, and benchmark gates in CI.

### Risk: Complexity creep

- Mitigation: phase-gate by milestone, each with green-state exit criteria and explicit compatibility contract.

## Open Questions

1. Should signature verification use a single canonical message hash model now or support multiple signature policy profiles at the runtime level?
2. How strict should default coercion be at first (warn-only or error) to avoid breaking exploratory workflows?
3. Should tx context be accepted as raw JSON, PSBT-derived JSON, or both with conversion adapters?
