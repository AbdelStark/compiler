# Arkade In-Process Runtime Plan

## Scope
Build a minimal, production-grade in-process VM inside this repository that executes compiled `ContractJson.functions[].asm` programs for tests, CLI usage, and interactive debugging.

## Ground Truth from Current Repo
- Artifact shape comes from `src/models/mod.rs` as `ContractJson` and `AbiFunction`.
- Example artifacts in `examples/*.json` expose `functions[].asm` token streams with opcodes and placeholders (for example `<sender>`, `<receiverSig>`, `144`).
- Current binary `arkadec` is compile-only (`src/main.rs`), so runtime commands must be added without breaking compile mode.
- Core opcode constants are in `src/opcodes/mod.rs`; examples actively use `OP_CHECKSIG`, `OP_SHA256`, `OP_EQUAL`, `OP_DROP`, `OP_DUP`, `OP_ADD64`, `OP_SUB64`, `OP_MUL64`, `OP_DIV64`, `OP_VERIFY`, and conditionals.

## Minimal File Plan
- `src/runtime/mod.rs` (module exports)
- `src/runtime/error.rs` (typed runtime errors)
- `src/runtime/value.rs` (stack value type + numeric conversions)
- `src/runtime/stack.rs` (`Stack` implementation for main/alt stacks)
- `src/runtime/env.rs` (`ExecutionEnv` and resolver traits)
- `src/runtime/dispatcher.rs` (`OpcodeDispatcher` and handlers)
- `src/runtime/vm.rs` (`VMState`, step loop, halt semantics)
- `src/runtime/telemetry.rs` (trace event model + tracing integration)
- `src/runtime/debugger.rs` (TUI event loop + panes)
- `src/main.rs` (subcommands: `compile`, `run`, `debug`)
- `src/lib.rs` (export runtime API)
- `tests/runtime_*` (new runtime unit and integration tests)

## Core Types

```rust
pub struct VMState {
    pub ip: usize,
    pub script: Vec<String>,
    pub stack: Stack,
    pub halted: bool,
    pub last_opcode: Option<String>,
    pub result: Option<ScriptOutcome>,
}

pub struct Stack {
    main: Vec<StackValue>,
    alt: Vec<StackValue>,
    max_depth: usize,
}

pub struct ExecutionEnv {
    pub bindings: std::collections::HashMap<String, StackValue>,
    pub tx_context: Option<TxContext>,
    pub checksig: std::sync::Arc<dyn ChecksigProvider + Send + Sync>,
    pub flags: VmFlags,
}

pub struct OpcodeDispatcher {
    handlers: std::collections::HashMap<&'static str, OpcodeHandler>,
}

pub type OpcodeHandler =
    fn(&mut VMState, &ExecutionEnv) -> Result<StepEffect, RuntimeError>;
```

Supporting enums:
- `StackValue`: `Int(i64)`, `Bytes(Vec<u8>)`, `Bool(bool)`, `Symbol(String)`.
- `ScriptOutcome`: `True`, `False`, `RuntimeError(RuntimeError)`.
- `StepEffect`: `Continue`, `Halt(ScriptOutcome)`.

## Runtime Semantics
- Any token not starting with `OP_` is a push token.
- Placeholder tokens like `<name>` resolve through `ExecutionEnv.bindings`; unresolved placeholders remain literal symbols unless strict mode is enabled.
- `OP_VERIFY` converts top item to bool; false yields script halt with `ScriptOutcome::False` (not runtime error).
- Unknown opcode or malformed stack/value yields `RuntimeError`.
- VM never panics on user script input; all failures are `Result`-based.

## Opcode Delivery Order
Phase A (core stack + flow):
- `OP_0..OP_16`, `OP_1NEGATE`, `OP_DUP`, `OP_DROP`, `OP_NIP`, `OP_NOT`, `OP_VERIFY`, `OP_IF`, `OP_ELSE`, `OP_ENDIF`.

Phase B (comparison + arithmetic):
- `OP_EQUAL`, `OP_EQUALVERIFY`, `OP_ADD64`, `OP_SUB64`, `OP_MUL64`, `OP_DIV64`, `OP_GREATERTHAN*`, `OP_LESSTHAN*`.

Phase C (crypto and checks):
- `OP_SHA256`, `OP_CHECKSIG`, `OP_CHECKSIGVERIFY`, `OP_CHECKMULTISIG` (mock provider).

Phase D (artifact-coverage opcodes):
- Timelock and introspection opcodes used by examples; unsupported opcodes return explicit `RuntimeError::UnsupportedOpcode`.

## Telemetry Stack
- Add dependencies: `tracing`, `tracing-subscriber`.
- Span model:
  - `vm.run` span: contract/function/variant metadata.
  - `vm.step` span per instruction: `ip`, `opcode_or_push`, pre/post stack depth, result.
- Levels:
  - `info!`: run start/finish and final outcome.
  - `debug!`: per-opcode stack transition snapshot.
  - `error!`: runtime errors with opcode/ip context.
- CLI `--trace` enables pretty subscriber output with full step logs.

## CLI Plan
- Convert current single-purpose CLI into subcommands:
  - `arkadec compile <file.ark> [-o out.json]` (existing behavior preserved).
  - `arkadec run <file.json> --function <name> --variant <true|false> [--trace]`.
  - `arkadec debug <file.json> --function <name> --variant <true|false>`.
- Exit code contract:
  - `0`: script success (`true`).
  - `1`: script halt false.
  - `2`: runtime/IO/parse error.

## TUI Plan (`ratatui` + `crossterm`)
- Add dependencies: `ratatui`, `crossterm` (runtime feature-gated as `debugger`).
- Pane layout:
  - Left: script list with current `ip` highlighted.
  - Right-top: main + alt stacks.
  - Right-bottom: telemetry/event log tail.
- Key bindings:
  - `n`: Step Over
  - `c`: Continue
  - `r`: Reset
  - `q`: Quit
- Debugger consumes the same VM engine step API as CLI run mode.

## Dependency Policy
- Required: `anyhow`, `tracing`, `tracing-subscriber`.
- Debug-only: `ratatui`, `crossterm`.
- No heavy crypto dependency for checksig initially; use deterministic mock provider trait and keep implementation swappable.

## Implementation Sequence
1. Introduce runtime value, error, stack primitives.
2. Add VM loop and dispatcher with stack/core opcodes.
3. Add arithmetic/comparison opcodes.
4. Add SHA256/checksig mocks.
5. Add artifact loader + function selector.
6. Add `arkadec run`.
7. Add telemetry wiring with trace-level stack logs.
8. Add TUI debugger bootstrap and controls.
9. Expand unsupported-opcode coverage for example artifacts.
10. Final repo-wide validation and docs update.

