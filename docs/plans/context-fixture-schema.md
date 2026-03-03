# Implementation Plan: Deterministic Context Fixture Schema and Ingestion

**Unit ID**: context-fixture-schema
**Bounded Context**: execution-context
**Category**: large
**Date**: 2026-03-03

---

## 1. Overview

This plan adds deterministic transaction context injection to the Arkade compiler's
execution pipeline. Users will supply a JSON fixture file (or inline JSON) containing
a full `TxContext` — txid, version, locktime, weight, inputs, outputs, assets, and
asset groups — and the runtime will use those values instead of the synthetic
`TxContext::sample()` defaults. The plan also adds a `--output json` mode producing a
versioned result envelope, and extends the WASM API with an optional `context_json`
parameter.

**Approach**: Create a new `src/runtime/context_fixture.rs` module with serde-backed
wire types (`TxContextWire`, `TxInputWire`, `TxOutputWire`, `AssetEntryWire`,
`AssetGroupWire`, `ContextFixture`) that deserialize from JSON and convert to the
existing runtime types. This keeps `env.rs` untouched (no serde derives on core types)
and provides clean separation.

---

## 2. TDD Applicability

**TDD applies.** This work adds:
- New CLI flags (`--context-file`, `--context-json`, `--context-strict`, `--output`)
- New public Rust module (`context_fixture.rs`) with deserialization + conversion logic
- New WASM API parameter (`context_json`)
- New JSON output format (`RunOutputEnvelope`)
- New observable behaviors verified by 5 BDD scenarios

All changes produce new observable behavior and new code paths. Tests are written
**before** each implementation step.

---

## 3. Step-by-Step Implementation

### Phase 0: Scaffold and Prerequisite Checks

| Step | Action |
|------|--------|
| 0.1 | Run `cargo build && cargo test` — confirm green baseline |
| 0.2 | Create `src/runtime/context_fixture.rs` (empty module) |
| 0.3 | Add `pub mod context_fixture;` to `src/runtime/mod.rs` |
| 0.4 | Run `cargo build` — confirm compilation with empty module |

### Phase 1: Wire Types and Deserialization (TDD)

**Goal**: `ContextFixture` JSON → `TxContext` conversion

#### 1a. Write unit tests FIRST

Create `tests/context_fixture_unit_test.rs`:

```rust
// Tests:
// - context_fixture_deserialize_minimal_fixture
//   Deserialize a JSON string with only txid + version, all other fields default.
//   Assert: tx_hash matches hex-decoded txid, version matches.
//
// - context_fixture_deserialize_full_fixture
//   Deserialize a complete fixture with inputs, outputs, asset_groups.
//   Assert: all fields round-trip correctly (lengths, values, hex decoding).
//
// - context_fixture_fallback_on_empty_string
//   Passing "" or "{}" returns TxContext::sample() equivalent.
//   Assert: tx_hash == SHA256("arkade-runtime-default-tx-hash").
//
// - context_fixture_strict_rejects_unknown_field [Scenario 3 unit]
//   Deserialize JSON with "rogue_field": 42 using deny_unknown_fields.
//   Assert: error message contains "rogue_field".
//
// - context_fixture_hex_decode_failure
//   Pass "txid": "ZZZZ" (invalid hex).
//   Assert: error describes hex decode failure.
```

#### 1b. Implement wire types

File: `src/runtime/context_fixture.rs`

```rust
use serde::Deserialize;
use crate::runtime::env::{AssetEntry, AssetGroup, TxContext, TxInput, TxOutput};

/// Strict variant rejects unknown fields.
/// Non-strict variant ignores them.
/// Two deserialization paths controlled by a boolean flag.

#[derive(Debug, Clone, Deserialize)]
pub struct AssetEntryWire {
    #[serde(default)]
    pub txid: String,           // hex
    #[serde(default)]
    pub gidx: u16,
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub data: String,           // hex
    #[serde(default)]
    pub control: String,        // hex
    #[serde(default)]
    pub metadata_hash: String,  // hex
    #[serde(default)]
    pub asset_id: String,       // hex
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxInputWire {
    #[serde(default)]
    pub value: i64,
    #[serde(default)]
    pub script_pubkey: String,  // hex
    #[serde(default)]
    pub sequence: i64,
    #[serde(default)]
    pub outpoint: String,       // hex
    #[serde(default)]
    pub issuance: String,       // hex
    #[serde(default)]
    pub assets: Vec<AssetEntryWire>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxOutputWire {
    #[serde(default)]
    pub value: i64,
    #[serde(default)]
    pub script_pubkey: String,  // hex
    #[serde(default)]
    pub nonce: String,          // hex
    #[serde(default)]
    pub assets: Vec<AssetEntryWire>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetGroupWire {
    #[serde(default)]
    pub txid: String,           // hex
    #[serde(default)]
    pub gidx: u16,
    #[serde(default)]
    pub sum_inputs: i64,
    #[serde(default)]
    pub sum_outputs: i64,
    #[serde(default)]
    pub num_inputs: i64,
    #[serde(default)]
    pub num_outputs: i64,
    #[serde(default)]
    pub control: String,        // hex
    #[serde(default)]
    pub metadata_hash: String,  // hex
    #[serde(default)]
    pub asset_id: String,       // hex
}

/// Non-strict variant (allows unknown fields — serde default behavior)
#[derive(Debug, Clone, Deserialize)]
pub struct ContextFixture {
    #[serde(default)]
    pub txid: String,                  // hex → maps to TxContext.tx_hash
    #[serde(default = "default_version")]
    pub version: i64,
    #[serde(default)]
    pub locktime: i64,
    #[serde(default)]
    pub weight: i64,
    #[serde(default)]
    pub current_input_index: usize,
    #[serde(default)]
    pub inputs: Vec<TxInputWire>,
    #[serde(default)]
    pub outputs: Vec<TxOutputWire>,
    #[serde(default)]
    pub asset_groups: Vec<AssetGroupWire>,
}

fn default_version() -> i64 { 2 }

/// Strict variant — rejects unknown fields
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFixtureStrict {
    #[serde(default)]
    pub txid: String,
    #[serde(default = "default_version")]
    pub version: i64,
    #[serde(default)]
    pub locktime: i64,
    #[serde(default)]
    pub weight: i64,
    #[serde(default)]
    pub current_input_index: usize,
    #[serde(default)]
    pub inputs: Vec<TxInputWire>,
    #[serde(default)]
    pub outputs: Vec<TxOutputWire>,
    #[serde(default)]
    pub asset_groups: Vec<AssetGroupWire>,
}

// Public API:
pub fn parse_context_json(json: &str, strict: bool) -> Result<TxContext, String>;
// - If json is empty or "{}": return TxContext::default() (synthetic fallback)
// - If strict: deserialize as ContextFixtureStrict (deny_unknown_fields)
// - Else: deserialize as ContextFixture (lenient)
// - Convert wire types → runtime types via hex::decode on all byte fields

impl ContextFixture {
    pub fn into_tx_context(self) -> Result<TxContext, String>;
    // hex-decode txid → tx_hash, recurse into inputs/outputs/asset_groups
}
```

#### 1c. Verify

```bash
cargo test --test context_fixture_unit_test
```

All 5 unit tests must pass before proceeding.

---

### Phase 2: CLI Flag Wiring (TDD)

**Goal**: Add `--context-file`, `--context-json`, `--context-strict`, `--output` flags

#### 2a. Write integration tests FIRST

Create `tests/runtime_cli_context_file_test.rs`:

```rust
// Scenario 1: run-with-context-file
// - Write a fixture JSON to a tempfile with txid "abcd1234..."
// - Run: arkadec run examples/htlc.json --function claim --context-file <tempfile>
// - Assert: exit code 0
// - Assert: (via --output json) envelope contains the fixture txid
//
// Scenario 2: run-without-context-falls-back
// - Run: arkadec run examples/htlc.json --function claim (no context flags)
// - Assert: exit code 0
// - Assert: behavior unchanged (existing test pattern)
```

Create `tests/runtime_cli_context_strict_test.rs`:

```rust
// Scenario 3: context-strict-rejects-unknown-fields
// - Write fixture JSON with "rogue_field": 42 to tempfile
// - Run: arkadec run examples/htlc.json --function claim
//        --context-file <tempfile> --context-strict
// - Assert: exit code non-zero (2)
// - Assert: stderr contains "rogue_field"
```

Create `tests/runtime_cli_json_output_test.rs`:

```rust
// Scenario 4: json-output-mode-parseable
// - Run: arkadec run examples/htlc.json --function claim --output json
// - Assert: exit code 0
// - Assert: stdout parses as serde_json::Value
// - Assert: JSON contains "status", "result", "schema_version" keys
// - Assert: schema_version == 1
```

#### 2b. Implement CLI changes

File: `src/main.rs`

**Add to `Command::Run` struct** (lines 52-70):

```rust
Run {
    file: String,
    #[arg(long)]
    function: String,
    #[arg(long, default_value = "false")]
    variant: String,
    #[arg(long = "bind")]
    bind: Vec<String>,
    #[arg(long, default_value_t = false)]
    trace: bool,
    #[arg(long, default_value_t = false)]
    strict: bool,
    // --- NEW FLAGS ---
    /// Path to a JSON fixture file supplying tx context
    #[arg(long)]
    context_file: Option<String>,
    /// Inline JSON string supplying tx context
    #[arg(long)]
    context_json: Option<String>,
    /// Reject fixture JSON with unknown fields
    #[arg(long, default_value_t = false)]
    context_strict: bool,
    /// Output format: "text" (default) or "json"
    #[arg(long, default_value = "text")]
    output: String,
},
```

**Extend `run_command` signature and logic** (lines 199-240):

```rust
fn run_command(
    file: &str,
    function: &str,
    variant: String,
    bind: Vec<String>,
    trace: bool,
    strict: bool,
    context_file: Option<String>,
    context_json: Option<String>,
    context_strict: bool,
    output_format: String,
) -> Result<i32> {
    init_tracing(trace);
    let variant = parse_bool_arg(&variant, "variant")?;

    let contract = load_contract_from_path(file)?;
    let program = runtime::load_program_from_contract(&contract, function, variant)?;

    // Resolve tx context
    let tx_context = resolve_tx_context(context_file, context_json, context_strict)?;

    let mut env = runtime::env::ExecutionEnv {
        strict_placeholders: strict,
        bindings: runtime::default_bindings_for_program(&program),
        tx_context,
    };
    env.bindings.extend(parse_bindings(&bind)?);

    let result = runtime::execute_program(&program, &env);

    if output_format == "json" {
        emit_json_output(&program, &result)?;
    } else {
        emit_text_output(&program, &result);
    }

    match result.outcome {
        runtime::vm::VmOutcome::ScriptTrue => Ok(0),
        runtime::vm::VmOutcome::ScriptFalse => Ok(1),
        runtime::vm::VmOutcome::RuntimeError(_) => Ok(2),
    }
}
```

**Add helper functions:**

```rust
fn resolve_tx_context(
    context_file: Option<String>,
    context_json_str: Option<String>,
    context_strict: bool,
) -> Result<runtime::env::TxContext> {
    let json_str = match (context_file, context_json_str) {
        (Some(path), _) => {
            fs::read_to_string(&path)
                .with_context(|| format!("failed reading context fixture '{path}'"))?
        }
        (_, Some(inline)) => inline,
        (None, None) => return Ok(runtime::env::TxContext::default()),
    };

    runtime::context_fixture::parse_context_json(&json_str, context_strict)
        .map_err(|e| anyhow::anyhow!("context fixture error: {e}"))
}
```

**Add `RunOutputEnvelope`:**

```rust
#[derive(serde::Serialize)]
struct RunOutputEnvelope {
    schema_version: u32,
    status: String,
    result: RunResultPayload,
}

#[derive(serde::Serialize)]
struct RunResultPayload {
    outcome: String,
    error_code: Option<String>,
    error_message: Option<String>,
}

fn emit_json_output(program: &runtime::LoadedProgram, result: &runtime::vm::VmRunResult) -> Result<()> {
    let (status, outcome, error_code, error_message) = match &result.outcome {
        runtime::vm::VmOutcome::ScriptTrue => ("true".into(), "script_true".into(), None, None),
        runtime::vm::VmOutcome::ScriptFalse => ("false".into(), "script_false".into(), None, None),
        runtime::vm::VmOutcome::RuntimeError(err) => {
            ("error".into(), "runtime_error".into(), None, Some(err.to_string()))
        }
    };

    let envelope = RunOutputEnvelope {
        schema_version: 1,
        status,
        result: RunResultPayload { outcome, error_code, error_message },
    };

    let json = serde_json::to_string_pretty(&envelope)?;
    println!("{json}");
    Ok(())
}
```

**Update `run()` match arm** to pass new fields through.

#### 2c. Verify

```bash
cargo test --test runtime_cli_context_file_test
cargo test --test runtime_cli_context_strict_test
cargo test --test runtime_cli_json_output_test
cargo test  # full suite green
```

---

### Phase 3: WASM API Extension (TDD)

**Goal**: Add optional `context_json` param to `execute_contract_json` and `execute_source`

#### 3a. Write WASM unit test FIRST

Add to existing WASM test infrastructure (or create `tests/wasm_context_test.rs` as a
library-level test that calls the non-wasm path of the same logic):

```rust
// Scenario 5: wasm-context-json-propagated
// - Construct a fixture JSON with txid = "deadbeef00" (padded to 32 bytes hex)
// - Call the library-level equivalent of execute_contract_json with context_json
// - Assert: the execution environment's tx_context.tx_hash matches decoded fixture txid
// - Assert: execution succeeds (no error returned)
```

Since WASM tests require `wasm-pack test` which may not be available in CI, we test
the context threading at the **library level** by extracting the context resolution
logic into a shared function callable from both WASM and native paths.

#### 3b. Implement WASM changes

File: `src/wasm.rs`

```rust
// execute_contract_json: add context_json as 6th parameter
#[wasm_bindgen]
pub fn execute_contract_json(
    contract_json: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
    context_json: &str,          // NEW — empty string = fallback
) -> Result<String, String> {
    // ... existing contract/program loading ...

    // NEW: resolve tx context from JSON
    let tx_context = if context_json.is_empty() {
        crate::runtime::env::TxContext::default()
    } else {
        crate::runtime::context_fixture::parse_context_json(context_json, false)?
    };

    let mut env = crate::runtime::env::ExecutionEnv {
        strict_placeholders,
        bindings: crate::runtime::default_bindings_for_program(&program),
        tx_context,    // was: ..ExecutionEnv::default()
    };
    env.bindings.extend(decode_bindings_json(bindings_json)?);

    // ... rest unchanged ...
}

// execute_source: thread context_json through to execute_contract_json
#[wasm_bindgen]
pub fn execute_source(
    source: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
    context_json: &str,          // NEW
) -> Result<String, String> {
    let contract = crate::compiler::compile(source)?;
    let contract_json = serde_json::to_string(&contract)
        .map_err(|err| format!("Serialization error: {err}"))?;
    execute_contract_json(
        &contract_json, function_name, server_variant,
        bindings_json, strict_placeholders, context_json,
    )
}
```

#### 3c. Update playground callers

File: `playground/index.html` (or `playground/app.js` if separate)

Add `""` as the 6th argument to all `execute_contract_json` / `execute_source` calls
so existing playground behavior is unchanged.

#### 3d. Verify

```bash
cargo test --test wasm_context_test
cargo build --features wasm  # if wasm target available
cargo test                   # full suite green
```

---

### Phase 4: Full Integration and Regression

| Step | Action |
|------|--------|
| 4.1 | Run `cargo fmt --check` |
| 4.2 | Run `cargo test` — full suite must be green |
| 4.3 | Manual smoke test: `cargo run -- run examples/htlc.json --function claim` (no context) |
| 4.4 | Manual smoke test: `cargo run -- run examples/htlc.json --function claim --output json` |
| 4.5 | Manual smoke test with fixture file |
| 4.6 | Manual smoke test: `--context-strict` with rogue field |

---

## 4. Scenario-to-Test Mapping

| Scenario ID | BDD Title | Test File(s) | Test Function(s) | Verify Command |
|-------------|-----------|--------------|-------------------|----------------|
| `run-with-context-file` | CLI run uses fixture tx context from file | `tests/runtime_cli_context_file_test.rs` | `cli_run_with_context_file_uses_fixture_txid` | `cargo test --test runtime_cli_context_file_test` |
| `run-without-context-falls-back` | CLI run with no context flags uses synthetic fallback | `tests/runtime_cli_context_file_test.rs` | `cli_run_without_context_uses_synthetic_fallback` | `cargo test --test runtime_cli_context_file_test` |
| `context-strict-rejects-unknown-fields` | context-strict mode rejects unknown fields | `tests/runtime_cli_context_strict_test.rs` | `cli_run_context_strict_rejects_rogue_field` | `cargo test --test runtime_cli_context_strict_test` |
| `json-output-mode-parseable` | JSON output mode emits valid structured result | `tests/runtime_cli_json_output_test.rs` | `cli_run_output_json_is_valid_envelope` | `cargo test --test runtime_cli_json_output_test` |
| `wasm-context-json-propagated` | WASM execute_contract_json propagates context | `tests/wasm_context_test.rs` | `wasm_execute_contract_json_propagates_fixture_txid` | `cargo test --test wasm_context_test` |

---

## 5. Files to Create

| File | Purpose |
|------|---------|
| `src/runtime/context_fixture.rs` | Wire types, `parse_context_json()`, `ContextFixture::into_tx_context()` |
| `tests/context_fixture_unit_test.rs` | Unit tests for deserialization and conversion |
| `tests/runtime_cli_context_file_test.rs` | CLI integration tests for Scenarios 1 & 2 |
| `tests/runtime_cli_context_strict_test.rs` | CLI integration test for Scenario 3 |
| `tests/runtime_cli_json_output_test.rs` | CLI integration test for Scenario 4 |
| `tests/wasm_context_test.rs` | Library-level test for Scenario 5 |

## 6. Files to Modify

| File | Changes |
|------|---------|
| `src/runtime/mod.rs` | Add `pub mod context_fixture;` (line 5 area) |
| `src/main.rs` | Add 4 new CLI flags to `Command::Run`; extend `run_command` signature; add `resolve_tx_context()`, `RunOutputEnvelope`, `emit_json_output()`, `emit_text_output()` |
| `src/wasm.rs` | Add `context_json: &str` param to `execute_contract_json` and `execute_source`; use `parse_context_json` for resolution |
| `playground/index.html` (or JS caller) | Add `""` as 6th arg to WASM calls for backward compat |

---

## 7. Key Function Signatures

```rust
// src/runtime/context_fixture.rs
pub fn parse_context_json(json: &str, strict: bool) -> Result<TxContext, String>;

// src/main.rs (updated)
fn run_command(
    file: &str, function: &str, variant: String, bind: Vec<String>,
    trace: bool, strict: bool,
    context_file: Option<String>, context_json: Option<String>,
    context_strict: bool, output_format: String,
) -> Result<i32>;

fn resolve_tx_context(
    context_file: Option<String>, context_json: Option<String>, context_strict: bool,
) -> Result<TxContext>;

fn emit_json_output(
    program: &LoadedProgram, result: &VmRunResult,
) -> Result<()>;

// src/wasm.rs (updated signatures)
pub fn execute_contract_json(
    contract_json: &str, function_name: &str, server_variant: bool,
    bindings_json: &str, strict_placeholders: bool, context_json: &str,
) -> Result<String, String>;

pub fn execute_source(
    source: &str, function_name: &str, server_variant: bool,
    bindings_json: &str, strict_placeholders: bool, context_json: &str,
) -> Result<String, String>;
```

---

## 8. No-Slop Constraints

### Minimum Assertions Per Test

| Test | Minimum Assertions |
|------|-------------------|
| Scenario 1 (context-file) | exit_code == 0 AND fixture txid appears in JSON output |
| Scenario 2 (fallback) | exit_code == 0 AND no behavioral change from existing test |
| Scenario 3 (strict) | exit_code != 0 AND stderr.contains("rogue_field") |
| Scenario 4 (JSON output) | stdout parses as JSON AND has "status" AND has "result" AND has "schema_version" AND schema_version == 1 |
| Scenario 5 (WASM) | execution succeeds AND tx_context.tx_hash matches fixture |

### Forbidden Shortcuts

- **No** adding serde derives directly to `TxContext`/`TxInput`/`TxOutput`/etc in `env.rs`
  (use wire types in `context_fixture.rs` instead)
- **No** new dependencies in `Cargo.toml` (serde, serde_json, hex already present)
- **No** modifying `TxContext::sample()` or `TxContext::default()`
- **No** changing `default_bindings_for_program` behavior (it uses its own TxContext internally)
- **No** making `context_json` parameter mandatory in WASM exports (empty string = fallback)
- **No** printing non-JSON content to stdout when `--output json` is active

### Verification Gates

| Gate | Command | Must |
|------|---------|------|
| Compilation | `cargo build` | Exit 0 |
| Format | `cargo fmt --check` | Exit 0 |
| Unit tests | `cargo test --test context_fixture_unit_test` | All pass |
| CLI integration | `cargo test --test runtime_cli_context_file_test --test runtime_cli_context_strict_test --test runtime_cli_json_output_test` | All pass |
| WASM integration | `cargo test --test wasm_context_test` | All pass |
| Full regression | `cargo test` | All pass (existing + new) |

---

## 9. Domain Invariant Validation

| Invariant | Where Validated |
|-----------|-----------------|
| Identical contract + context + bindings → identical result | `tests/context_fixture_unit_test.rs`: deserialize same fixture twice → same TxContext; `tests/runtime_cli_context_file_test.rs`: same fixture → same exit code |
| Absence of context flags → no observable change | `tests/runtime_cli_context_file_test.rs::cli_run_without_context_uses_synthetic_fallback` — matches existing test output exactly; existing `runtime_cli_run_test.rs` tests remain unmodified and passing |

---

## 10. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| WASM parameter change breaks playground | Playground JS calls fail at runtime | Add `""` default to all playground callers; document in commit message |
| `deny_unknown_fields` also applies to nested types | Nested `TxInputWire` etc. might reject unknown fields when not desired | Only apply `deny_unknown_fields` to top-level `ContextFixtureStrict`; nested types remain lenient (serde default) unless strict variant created for them too |
| Hex decode errors from malformed fixture | Runtime crash or confusing error | `parse_context_json` returns descriptive `Result::Err` with field name and hex error |
| `default_bindings_for_program` uses separate `TxContext::default()` | `_txid`/`_gidx` binding defaults won't match user fixture | Document as known limitation; binding inference uses synthetic context regardless of user fixture |
| `--context-file` and `--context-json` both supplied | Ambiguous intent | Error if both are present; enforce mutual exclusivity in CLI validation |
| `--output json` mode must not emit trace logs to stdout | Tracing subscriber writes to stderr by default ✓ | Verify trace output goes to stderr only; `--output json` emits only the envelope to stdout |

---

## 11. Verification Against Acceptance Criteria

| AC# | Criterion | How Verified |
|-----|-----------|-------------|
| 1 | `cargo build` succeeds; `cargo test` green | Gate: `cargo build && cargo test` after each phase |
| 2 | `--context-file fixture.json` uses fixture txid | `runtime_cli_context_file_test::cli_run_with_context_file_uses_fixture_txid` |
| 3 | No context flags → synthetic fallback | `runtime_cli_context_file_test::cli_run_without_context_uses_synthetic_fallback` + existing `runtime_cli_run_test.rs` unchanged |
| 4 | WASM `execute_contract_json` accepts context_json | `wasm_context_test::wasm_execute_contract_json_propagates_fixture_txid` |
| 5 | `--output json` produces parseable envelope | `runtime_cli_json_output_test::cli_run_output_json_is_valid_envelope` |
| 6 | `--context-strict` rejects unknown field | `runtime_cli_context_strict_test::cli_run_context_strict_rejects_rogue_field` |

---

## 12. Implementation Order Summary

```
Phase 0: Scaffold
  0.1  cargo build && cargo test (baseline)
  0.2  Create empty context_fixture.rs
  0.3  Register module in runtime/mod.rs
  0.4  cargo build (verify)

Phase 1: Wire Types (TDD)
  1.1  Write context_fixture_unit_test.rs (5 tests — all RED)
  1.2  Implement wire types + parse_context_json + into_tx_context
  1.3  cargo test --test context_fixture_unit_test (all GREEN)

Phase 2: CLI Flags (TDD)
  2.1  Write runtime_cli_context_file_test.rs (Scenarios 1,2 — RED)
  2.2  Write runtime_cli_context_strict_test.rs (Scenario 3 — RED)
  2.3  Write runtime_cli_json_output_test.rs (Scenario 4 — RED)
  2.4  Implement CLI flag additions + run_command changes + RunOutputEnvelope
  2.5  cargo test (all GREEN)

Phase 3: WASM Extension (TDD)
  3.1  Write wasm_context_test.rs (Scenario 5 — RED)
  3.2  Implement WASM parameter changes
  3.3  Update playground callers
  3.4  cargo test (all GREEN)

Phase 4: Regression & Format
  4.1  cargo fmt --check
  4.2  cargo test (full suite)
  4.3  Manual smoke tests
```
