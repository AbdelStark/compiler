# Research: Deterministic Context Fixture Schema and Ingestion

**Unit ID**: context-fixture-schema
**Bounded Context**: execution-context
**Category**: large
**Date**: 2026-03-03

---

## 1. Ubiquitous Language Definitions

| Term | Definition in this context |
|------|---------------------------|
| **fixture** | A deterministic JSON file/string supplying a full `TxContext` for contract execution |
| **tx-context** | The in-memory `TxContext` struct holding txid, version, locktime, weight, inputs, outputs, asset_groups |
| **execution-envelope** | The versioned JSON output object wrapping status, result, and schema_version from a `run` invocation |
| **deterministic-execution** | Property: identical contract + context + bindings always produce identical outcome |
| **synthetic-fallback** | `TxContext::sample()` — current default when no fixture is supplied; must be preserved |

---

## 2. Domain Model

### Aggregates (✓ VERIFIED from `src/runtime/env.rs`)

#### `TxContext` (runtime aggregate root for transaction state)
```rust
pub struct TxContext {
    pub tx_hash: Vec<u8>,          // NOTE: wire name is "txid"; runtime is "tx_hash"
    pub version: i64,
    pub locktime: i64,
    pub weight: i64,
    pub current_input_index: usize,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub asset_groups: Vec<AssetGroup>,
}
```
**No serde derives** — currently pure runtime type. Must add serde support or create wire types.

#### `ExecutionEnv` (aggregate root for execution)
```rust
pub struct ExecutionEnv {
    pub bindings: HashMap<String, StackValue>,
    pub strict_placeholders: bool,
    pub tx_context: TxContext,
}
```
Default via `ExecutionEnv::default()` → `ExecutionEnv::new()` → `TxContext::default()` → `TxContext::sample()`.

### Value Objects (✓ VERIFIED from `src/runtime/env.rs`)

#### `TxInput`
```rust
pub struct TxInput {
    pub value: i64,
    pub script_pubkey: Vec<u8>,
    pub sequence: i64,
    pub outpoint: Vec<u8>,
    pub issuance: Vec<u8>,
    pub assets: Vec<AssetEntry>,
}
```

#### `TxOutput`
```rust
pub struct TxOutput {
    pub value: i64,
    pub script_pubkey: Vec<u8>,
    pub nonce: Vec<u8>,
    pub assets: Vec<AssetEntry>,
}
```

#### `AssetEntry`
```rust
pub struct AssetEntry {
    pub txid: Vec<u8>,
    pub gidx: u16,
    pub amount: i64,
    pub data: Vec<u8>,
    pub control: Vec<u8>,
    pub metadata_hash: Vec<u8>,
    pub asset_id: Vec<u8>,
}
```

#### `AssetGroup`
```rust
pub struct AssetGroup {
    pub txid: Vec<u8>,
    pub gidx: u16,
    pub sum_inputs: i64,
    pub sum_outputs: i64,
    pub num_inputs: i64,
    pub num_outputs: i64,
    pub control: Vec<u8>,
    pub metadata_hash: Vec<u8>,
    pub asset_id: Vec<u8>,
}
```

### Domain Invariants
1. **Determinism**: identical `(contract, context, bindings)` triple → identical `VmRunResult.outcome`
2. **Backward compatibility**: absence of new context flags must not alter any existing observable behavior — `TxContext::default()` (synthetic fallback) must remain unchanged

---

## 3. Current Implementation State

### 3.1 CLI `run` subcommand (✓ VERIFIED from `src/main.rs:52-70`)

```rust
Run {
    file: String,
    function: String,
    variant: String,
    bind: Vec<String>,
    trace: bool,
    strict: bool,           // maps to strict_placeholders in ExecutionEnv
}
```

**Missing flags** (to add):
- `--context-file <PATH>` → load fixture JSON from file
- `--context-json <STR>` → load fixture JSON inline
- `--context-strict` → deny unknown fields on fixture deserialization
- `--output <FORMAT>` → `json` (or default plain text)

### 3.2 `run_command` execution flow (✓ VERIFIED from `src/main.rs:199-240`)

Current flow:
1. Load contract via `load_contract_from_path`
2. Load program via `runtime::load_program_from_contract`
3. Build `ExecutionEnv` — bindings from `default_bindings_for_program` + CLI overrides
4. `..ExecutionEnv::default()` — uses `TxContext::sample()` as tx_context
5. `runtime::execute_program` → `VmRunResult`
6. Print plain-text outcome

New flow adds step 3.5: if `--context-file` or `--context-json` present → deserialize `ContextFixture` → replace `env.tx_context`.

### 3.3 WASM APIs (✓ VERIFIED from `src/wasm.rs:196-240`)

```rust
pub fn execute_contract_json(
    contract_json: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
) -> Result<String, String>

pub fn execute_source(
    source: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
) -> Result<String, String>
```

**Missing**: optional `context_json: &str` parameter for both functions.
Note: `execute_source` delegates to `execute_contract_json` — only need to thread context through the chain.

### 3.4 Synthetic fallback (✓ VERIFIED from `src/runtime/env.rs:84-194`)

`TxContext::sample()` generates deterministic hash-based fixtures:
- 4 asset groups (hash-derived txids, gidx 0–3)
- 4 inputs, 4 outputs (each with 4 embedded assets)
- `tx_hash` = SHA256("arkade-runtime-default-tx-hash")
- version=2, locktime=0, weight=850

**This must be preserved exactly.** New code only overrides via explicit fixture supply.

### 3.5 `default_bindings_for_program` coupling (✓ VERIFIED from `src/runtime/mod.rs:86-178`)

The binding inference function uses `TxContext::default()` internally (line 88) to derive asset-related placeholder values. This is a **separate** `TxContext` instance from `ExecutionEnv.tx_context`. When a fixture is supplied, the `default_bindings_for_program` path still uses the synthetic context for binding defaults — only `env.tx_context` is replaced. This creates a potential inconsistency if fixture txids differ from synthetic defaults in binding heuristics.

**Risk**: `_txid` and `_gidx` placeholder defaults are derived from `TxContext::default()`, not from the user-supplied fixture. May need to thread fixture into binding inference or document this limitation.

---

## 4. New Types Required

### 4.1 `ContextFixture` (new, `src/runtime/context_fixture.rs`)

Wire type for JSON deserialization. Bytes as hex strings.

```json
{
  "tx_context": {
    "txid": "0xabcd1234...",
    "version": 2,
    "locktime": 0,
    "weight": 850,
    "current_input_index": 0,
    "inputs": [
      {
        "value": 100000,
        "script_pubkey": "hex...",
        "sequence": 4294967280,
        "outpoint": "hex...",
        "issuance": "hex...",
        "assets": [
          {
            "txid": "hex...",
            "gidx": 0,
            "amount": 5000,
            "data": "hex...",
            "control": "hex...",
            "metadata_hash": "hex...",
            "asset_id": "hex..."
          }
        ]
      }
    ],
    "outputs": [
      {
        "value": 99000,
        "script_pubkey": "hex...",
        "nonce": "hex...",
        "assets": []
      }
    ],
    "asset_groups": [
      {
        "txid": "hex...",
        "gidx": 0,
        "sum_inputs": 10000,
        "sum_outputs": 9500,
        "num_inputs": 2,
        "num_outputs": 2,
        "control": "hex...",
        "metadata_hash": "hex...",
        "asset_id": "hex..."
      }
    ]
  }
}
```

**Key naming note**: JSON field `txid` maps to runtime field `tx_hash`.

### 4.2 `RunOutputEnvelope` (new, `src/main.rs` or separate module)

```json
{
  "schema_version": 1,
  "status": "true",
  "result": {
    "outcome": "script_true",
    "error_code": null,
    "error_message": null
  }
}
```

Required keys per BDD Scenario 4: `status`, `result`, `schema_version`.

---

## 5. Scenario Impact Map

### Scenario 1: `run-with-context-file`
**Code paths**:
1. `src/main.rs`: Add `context_file: Option<String>` to `Command::Run`
2. `src/main.rs:run_command`: Parse `--context-file`, load file, deserialize `ContextFixture`
3. **New**: `src/runtime/context_fixture.rs`: `ContextFixture` + conversion to `TxContext`
4. `src/runtime/env.rs`: `TxContext` receives converted fixture value
5. **New test**: `tests/runtime_cli_context_file_test.rs` — spawn `arkadec run ... --context-file fixture.json`, assert exit 0 and txid propagation (may need runtime introspection opcode path for txid verification, OR test via debug output)

**OPEN**: The CLI currently prints "RESULT: true/false/runtime_error" — txid propagation verification requires either: (a) a contract that reads `tx.hash` and checks a known value, or (b) adding txid to output. The BDD says "execution environment txid equals '0xabcd1234'" but the current output doesn't expose txid. May need `--output json` for this assertion.

### Scenario 2: `run-without-context-falls-back`
**Code paths**:
1. No new code paths — existing `TxContext::default()` fallback
2. Regression test: existing `runtime_cli_run_test.rs::cli_run_succeeds_for_htlc_claim_exit_variant` already covers this
3. May add explicit test: `cli_run_without_context_succeeds` to confirm no regression

### Scenario 3: `context-strict-rejects-unknown-fields`
**Code paths**:
1. `src/main.rs`: Add `context_strict: bool` to `Command::Run`
2. `src/runtime/context_fixture.rs`: `deserialize_strict(json: &str)` using `serde(deny_unknown_fields)` OR custom serde error handling
3. Error message must name the unknown field (e.g., `"rogue_field"`)
4. **New test**: `tests/runtime_cli_context_strict_test.rs` — assert non-zero exit, stderr contains "rogue_field"

**Implementation note**: `serde(deny_unknown_fields)` automatically names the unexpected field in the error message. This satisfies the diagnostic requirement cheaply.

### Scenario 4: `json-output-mode-parseable`
**Code paths**:
1. `src/main.rs`: Add `output_mode: Option<String>` to `Command::Run` with `--output` flag
2. `src/main.rs:run_command`: Branch on `output_mode == "json"` → emit `RunOutputEnvelope` as JSON to stdout
3. **New test**: `tests/runtime_cli_json_output_test.rs` — parse stdout as JSON, assert keys present

**Important**: Must emit ONLY valid JSON to stdout (no extra text) when `--output json` is set.

### Scenario 5: `wasm-context-json-propagated`
**Code paths**:
1. `src/wasm.rs`: Add `context_json: &str` to `execute_contract_json` signature
2. `src/wasm.rs`: Deserialize `TxContextWire` from `context_json` (or empty → fallback)
3. `src/wasm.rs`: Thread into `ExecutionEnv.tx_context`
4. `src/wasm.rs`: `execute_source` passes through to `execute_contract_json`
5. **New test**: WASM tests are harder — likely a `#[test]` in `src/wasm.rs` using `#[cfg(test)]` or a dedicated playground integration test

**Important**: WASM context_json is separate from bindings_json. The bindings are caller-side values for `<placeholder>` tokens; the context is the tx environment. Keep them as separate parameters.

---

## 6. Critical Implementation Decisions

### Decision 1: Wire Types vs Derive on Runtime Types
**Options**:
- A) Add `#[derive(Serialize, Deserialize)]` directly to `TxContext`, `TxInput`, etc. in `env.rs` — simpler, but these are `gated` files (supervised zone per CLAUDE.md)
- B) Create `TxContextWire`, `TxInputWire`, etc. in a new file `context_fixture.rs` — more code but cleaner separation

**Recommendation**: Option B — wire types in new `src/runtime/context_fixture.rs`. The runtime types are in a supervised file (`src/runtime/env.rs` — wait, actually `env.rs` is not listed as supervised in `CLAUDE.md`; only `compiler/mod.rs`, `models/mod.rs`, `grammar.pest`, `Cargo.toml`, `.github/workflows/`, `scripts/pre-commit` are supervised). However, adding serde to `env.rs` may have minor impact. Wire types in a dedicated file are cleaner.

### Decision 2: Hex encoding for `Vec<u8>` fields
All byte fields (`txid`, `script_pubkey`, `outpoint`, `issuance`, `data`, `control`, `metadata_hash`, `asset_id`, `nonce`) should be hex-encoded strings in the JSON fixture. Use `hex::encode`/`hex::decode` for conversion. The existing `wasm.rs` uses this pattern (see `StackWireInput::BytesHex`).

### Decision 3: `current_input_index` in fixture
The `TxContext.current_input_index` is used by the VM's `OP_INPUTINDEX` opcode. Should be exposed in the fixture schema. Default to 0 if omitted.

### Decision 4: `context_json` as empty string = fallback
The WASM API convention (see existing `bindings_json` handling in `wasm.rs:176-188`) uses empty string as "no value, use default". Apply same pattern for `context_json`.

### Decision 5: Structural fixture validation
`--context-strict` only enforces no unknown fields. It should NOT require all fields to be present (partial fixtures are useful). Use `serde(deny_unknown_fields)` with all fixture fields marked `#[serde(default)]`.

---

## 7. Files to Review (Complete Scope)

| File | Change Type | Zone |
|------|-------------|------|
| `src/main.rs` | Modify — add CLI flags to `Command::Run`, extend `run_command` | autonomous |
| `src/runtime/context_fixture.rs` | Create — wire types, deserialization, conversion | autonomous |
| `src/runtime/mod.rs` | Modify — register `context_fixture` module export | autonomous |
| `src/wasm.rs` | Modify — add `context_json` param to WASM functions | autonomous |
| `tests/context_fixture_unit_test.rs` | Create — unit coverage for parse/strict/fallback/hex errors | autonomous |
| `tests/runtime_cli_context_file_test.rs` | Create — Scenario 1 integration test | autonomous |
| `tests/runtime_cli_context_strict_test.rs` | Create — Scenario 3 integration test | autonomous |
| `tests/runtime_cli_json_output_test.rs` | Create — Scenario 4 integration test | autonomous |
| `tests/wasm_context_test.rs` | Create — Scenario 5 context propagation coverage | autonomous |

---

## 8. Dependency Notes

- `serde` + `serde_json` already in `Cargo.toml` ✓ VERIFIED
- `hex` already in `Cargo.toml` ✓ VERIFIED
- `tempfile` in `[dev-dependencies]` for temp fixture files in tests ✓ VERIFIED
- No new dependencies required

---

## 9. Test Patterns from Existing Code (✓ VERIFIED)

From `tests/runtime_cli_run_test.rs`:
```rust
let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
    .arg("run")
    .arg("examples/htlc.json")
    .arg("--function").arg("claim")
    .arg("--variant").arg("false")
    .output()
    .expect("failed to run cli");

assert!(output.status.success());
let stdout = String::from_utf8_lossy(&output.stdout);
assert!(stdout.contains("RESULT: true"), "stdout was: {stdout}");
```

New tests use same `Command::new(env!("CARGO_BIN_EXE_arkadec"))` pattern with added args. Temp files via `tempfile::NamedTempFile` for fixture JSON.

---

## 10. Open Questions

1. **Txid propagation verification in Scenario 1**: The BDD asserts "execution environment txid equals '0xabcd1234'" — but the current run output doesn't expose txid. How to verify? Options:
   - Use a contract that introspects tx hash and asserts a specific value (fragile)
   - Use `--output json` and include txid in the envelope (adds coupling between features)
   - Add a separate `--dump-context` diagnostic flag (deferred)
   - Accept that the integration test verifies "no error" and the unit test (library-level) verifies the txid field

2. **`default_bindings_for_program` inconsistency**: The binding inference uses `TxContext::default()` internally (not the user fixture). Should it use the provided fixture context for `_txid` / `_gidx` defaults? Likely a follow-on improvement, but should be documented.

3. **WASM breaking change**: Adding `context_json` as a positional parameter to `execute_contract_json` and `execute_source` is a **breaking change** for existing playground callers. Should it be added as the last parameter (least disruptive) or as a new overloaded function? The RFC says "extend... to accept an optional `context_json`" — recommend adding as the last parameter to minimize breakage.

4. **`current_input_index` in fixture**: Should this be in the top-level `tx_context` object or omitted and always default to 0? If a fixture should test specific input index scenarios, it must be exposed.

5. **Asset group vs per-input assets relationship**: `TxContext.inputs[i].assets` and `TxContext.asset_groups` are redundant in the sample (they're the same 4 assets). In a real fixture, they could differ. Should the fixture validate consistency between them?
