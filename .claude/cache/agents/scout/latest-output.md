# Codebase Report: Arkade Compiler - Runtime, CLI, and WASM Structure
Generated: 2026-03-03

## Summary

The Arkade compiler is a single-crate Rust project that compiles `.ark` contracts into JSON ABI + script assembly.
The runtime executes those artifacts in-process (VM) or via CLI (`arkadec run`). A WASM bridge exposes compile + execute
to browsers. The project uses `secp256k1` for crypto on native (non-WASM) targets and a SHA256-based stub on WASM.

---

## Project Structure

```
src/
  lib.rs                  # Public crate API (compile, re-exports)
  main.rs                 # CLI entrypoint (arkadec) — Compile / Run / Debug subcommands
  wasm.rs                 # WASM bindings (feature = "wasm")
  compiler/mod.rs         # AST → ABI / ASM generation (gated)
  models/mod.rs           # ABI JSON structs + AST structs
  opcodes/mod.rs          # Opcode constants
  parser/
    grammar.pest          # PEG grammar (gated)
    mod.rs                # Pest → AST parser
  runtime/
    mod.rs                # load_program_*, execute_program, default_bindings_for_program
    env.rs                # ExecutionEnv, TxContext, TxInput, TxOutput, AssetEntry, AssetGroup
    vm.rs                 # VMState, VmRunResult, VmOutcome
    stack.rs              # Stack operations
    value.rs              # StackValue enum
    error.rs              # RuntimeError, RuntimeErrorCode
    dispatcher.rs         # Opcode dispatch
    telemetry.rs          # Execution step recording
    debugger.rs           # TUI debugger (non-WASM only)
tests/
  parity_support/mod.rs   # Shared fixture loading helpers
  parity_vectors/         # 012 JSON vector files
  parity_vectors_introspector/  # 7 more JSON vector files
  runtime_*.rs            # Unit/integration test files
  *.rs                    # Contract-level integration tests
```

---

## Questions Answered

---

### Q1: `src/runtime/env.rs` — Full Structs and Implementations

**File:** `src/runtime/env.rs` (367 lines)

#### Structs

```rust
// Lines 30-38
pub struct AssetEntry {
    pub txid: Vec<u8>,
    pub gidx: u16,
    pub amount: i64,
    pub data: Vec<u8>,
    pub control: Vec<u8>,
    pub metadata_hash: Vec<u8>,
    pub asset_id: Vec<u8>,
}

// Lines 41-48
pub struct TxInput {
    pub value: i64,
    pub script_pubkey: Vec<u8>,
    pub sequence: i64,
    pub outpoint: Vec<u8>,
    pub issuance: Vec<u8>,
    pub assets: Vec<AssetEntry>,
}

// Lines 51-56
pub struct TxOutput {
    pub value: i64,
    pub script_pubkey: Vec<u8>,
    pub nonce: Vec<u8>,
    pub assets: Vec<AssetEntry>,
}

// Lines 59-69
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

// Lines 72-81
pub struct TxContext {
    pub tx_hash: Vec<u8>,
    pub version: i64,
    pub locktime: i64,
    pub weight: i64,
    pub current_input_index: usize,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    pub asset_groups: Vec<AssetGroup>,
}

// Lines 196-201
pub struct ExecutionEnv {
    pub bindings: HashMap<String, StackValue>,
    pub strict_placeholders: bool,
    pub tx_context: TxContext,
}
```

#### `TxContext::sample()` (Lines 84-167)

Builds a deterministic sample context:
- 4 `AssetGroup`s — `txid` = SHA256("asset-group-txid-{idx}"), `gidx` = idx (0..3),
  `sum_inputs` = 10_000 + idx*100, `sum_outputs` = 9_500 + idx*100,
  `num_inputs` = 2, `num_outputs` = 2.
  `asset_id` = SHA256(txid ++ gidx.to_le_bytes())
- 4 `TxInput`s — `value` = 100_000+i, `sequence` = 0xFFFF_FFF0+i,
  `outpoint` = SHA256("outpoint-{i}"), each carrying all 4 input_assets.
- 4 `TxOutput`s — `value` = 99_000+i, `nonce` = SHA256("nonce-{i}"),
  each carrying all 4 output_assets.
- `tx_hash` = SHA256("arkade-runtime-default-tx-hash"), `version` = 2,
  `locktime` = 0, `weight` = 850.

#### `TxContext::default()` (Lines 190-194)

```rust
impl Default for TxContext {
    fn default() -> Self { Self::sample() }
}
```

#### `ExecutionEnv::new()` / `Default` (Lines 203-213, 349-353)

```rust
pub fn new() -> Self {
    Self {
        bindings: HashMap::new(),
        strict_placeholders: false,
        tx_context: TxContext::default(),
    }
}
impl Default for ExecutionEnv {
    fn default() -> Self { Self::new() }
}
```

#### `stack_value_to_bytes` (Lines 355-366)

Free function; converts `StackValue` → `Vec<u8>`:
- `Bytes(v)` → clone
- `Int(v)` → 8-byte LE
- `Bool(v)` → `[u8::from(v)]`
- `Symbol(v)` → hex-decode if even-length all-hex, else UTF-8 bytes

---

### Q2: `src/runtime/mod.rs` — Key Public Functions

**File:** `src/runtime/mod.rs` (411 lines)

#### `LoadedProgram` struct (Lines 24-30)

```rust
pub struct LoadedProgram {
    pub contract_name: String,
    pub function_name: String,
    pub server_variant: bool,
    pub asm: Vec<String>,
    pub param_types: HashMap<String, String>,
}
```

#### `load_program_from_file` (Lines 32-56)

Reads a `.json` artifact from disk, deserializes it, delegates to `load_program_from_contract`.

#### `load_program_from_contract` (Lines 58-79)

Finds the matching `AbiFunction` (by name + serverVariant), collects param types from
both contract-level `parameters` and function-level `function_inputs`, returns `LoadedProgram`.

#### `execute_program` (Lines 81-84)

```rust
pub fn execute_program(program: &LoadedProgram, env: &ExecutionEnv) -> VmRunResult {
    let mut vm = VMState::new(program.asm.clone());
    vm.run(env)
}
```

#### `default_bindings_for_program` (Lines 86-179)

Scans ASM for placeholders (`<name>` tokens), then applies this binding strategy:
1. Typed defaults from `param_types` (`pubkey` → derived pubkey bytes, `signature` → 64 zero bytes,
   `bytes32` → 32 zero bytes, `bytes` → empty, `int`/`value` → 0, `bool` → true)
2. Special names: `preimage` → b"hello", `hash` → SHA256(b"hello")
3. `*_txid` suffix → `TxContext.asset_groups[hash_to_index(name)].txid`
4. `*_gidx` suffix → `TxContext.asset_groups[hash_to_index(name)].gidx`
5. Probable numeric names (contain: time, value, amount, count, index, group; or `i`/`j`/`k`) → `Int(0)`
6. Everything else → `Symbol(name)` (then coerced to `Int(0)` at end)
7. Auto-signs: scans for CHECKSIG / CHECKSIGFROMSTACK / CHECKSIGADD / CHECKMULTISIG patterns,
   derives deterministic keypairs per label, signs with `tx_context.tx_hash` (or explicit message).
8. Ensures `refundTime = Int(0)` is always present.

---

### Q3: `src/main.rs` — CLI Arg Parsing

**File:** `src/main.rs` (505 lines)

#### Clap struct hierarchy

```rust
#[derive(ClapParser, Debug)]
#[command(name = "arkadec")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    // Legacy positional compile mode
    file: Option<String>,
    #[arg(short, long)]
    output: Option<String>,
}

enum Command {
    Compile {
        file: String,
        #[arg(short, long)] output: Option<String>,
    },
    Run {
        file: String,
        #[arg(long)] function: String,
        #[arg(long, default_value = "false")] variant: String,
        #[arg(long = "bind")] bind: Vec<String>,   // key=value, value can be: 42 / true / hex:DEADBEEF / symbol
        #[arg(long, default_value_t = false)] trace: bool,
        #[arg(long, default_value_t = false)] strict: bool,
    },
    Debug {
        file: Option<String>,
        #[arg(long)] function: Option<String>,
        #[arg(long)] variant: Option<String>,
        #[arg(long = "bind")] bind: Vec<String>,
        #[arg(long, default_value_t = false)] trace: bool,
        #[arg(long, default_value_t = false)] strict: bool,
        #[arg(long = "breakpoint")] breakpoint: Vec<usize>,
        #[arg(long, default_value_t = false)] list_samples: bool,
        #[arg(long, default_value_t = false)] pick: bool,
        #[arg(long, default_value_t = false)] headless: bool,
    },
}
```

#### `run_command` (Lines 199-240)

1. Parses `variant` string → bool.
2. Loads contract via `load_contract_from_path` (accepts `.json` or `.ark`).
3. Builds `ExecutionEnv` with `default_bindings_for_program`, then extends with `parse_bindings`.
4. Calls `runtime::execute_program`.
5. Prints `RESULT: true` (exit 0), `RESULT: false` (exit 1), or `RESULT: runtime_error` (exit 2).

#### `parse_binding_value` (Lines 331-347)

- `"true"`/`"false"` → `Bool`
- integer string → `Int`
- `"hex:XXXX"` → `Bytes` (hex-decode)
- anything else → `Symbol`

#### `load_contract_from_path` (Lines 367-396)

Accepts `.json` (deserialize) or `.ark` (compile-then-use). Bails on other extensions.

---

### Q4: `src/wasm.rs` — WASM Exports

**File:** `src/wasm.rs` (240 lines)

#### Exported functions

| Symbol | Signature | Notes |
|--------|-----------|-------|
| `init` | `fn init()` | `#[wasm_bindgen(start)]` — installs panic hook |
| `compile` | `fn compile(source: &str) -> Result<String, String>` | Returns pretty JSON |
| `version` | `fn version() -> String` | Cargo pkg version |
| `validate` | `fn validate(source: &str) -> Result<bool, String>` | Compiles and discards output |
| `execute_contract_json` | `fn(contract_json, function_name, server_variant, bindings_json, strict_placeholders) -> Result<String, String>` | Main execute path |
| `execute_source` | `fn(source, function_name, server_variant, bindings_json, strict_placeholders) -> Result<String, String>` | Compile-then-execute |

#### Wire types

**Input bindings JSON** (`bindings_json`):
```json
{
  "myKey": { "type": "int",        "value": 42       },
  "myKey": { "type": "bool",       "value": true      },
  "myKey": { "type": "bytes_hex",  "value": "deadbeef" },
  "myKey": { "type": "bytes_utf8", "value": "hello"   },
  "myKey": { "type": "symbol",     "value": "foo"     }
}
```

**Output JSON** (`RuntimeExecutionWire`):
```json
{
  "contract_name": "...",
  "function_name": "...",
  "server_variant": false,
  "outcome": "script_true|script_false|runtime_error",
  "error_code": null,
  "error_message": null,
  "final_main_stack": [{"type": "int|bool|bytes_hex|symbol", "value": ...}],
  "final_alt_stack": [],
  "telemetry": [
    {"ip": 0, "token": "OP_FOO", "status": "continue|script_true|...",
     "stack_before": [...], "stack_after": [...]}
  ]
}
```

Both `execute_contract_json` and `execute_source`:
1. Parse/compile the contract.
2. Call `load_program_from_contract`.
3. Build `ExecutionEnv` with `default_bindings_for_program`, extend with decoded bindings.
4. Call `execute_program`.
5. Serialize `RuntimeExecutionWire` → pretty JSON.

---

### Q5: `src/models/mod.rs` — ABI and Output Models

**File:** `src/models/mod.rs` (320 lines)

#### ABI / JSON models

```rust
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type")] pub param_type: String,
}

pub struct FunctionInput {
    pub name: String,
    #[serde(rename = "type")] pub param_type: String,
}

pub struct RequireStatement {
    #[serde(rename = "type")] pub req_type: String,
    pub message: Option<String>,
}

pub struct AbiFunction {
    pub name: String,
    #[serde(rename = "functionInputs")] pub function_inputs: Vec<FunctionInput>,
    #[serde(rename = "serverVariant")]  pub server_variant: bool,
    pub require: Vec<RequireStatement>,
    pub asm: Vec<String>,
}

pub struct ContractJson {
    #[serde(rename = "contractName")]   pub name: String,
    #[serde(rename = "constructorInputs")] pub parameters: Vec<Parameter>,
    pub functions: Vec<AbiFunction>,
    pub source: Option<String>,
    pub compiler: Option<CompilerInfo>,
    #[serde(rename = "updatedAt")]      pub updated_at: Option<String>,
}

pub struct CompilerInfo { pub name: String, pub version: String }
```

#### AST models (not serialized)

`Contract`, `Function`, `Statement` (Require, LetBinding, VarAssign, IfElse, ForIn),
`Requirement` (CheckSig, CheckSigFromStack, CheckMultisig, After, HashEqual, Comparison),
`Expression` (40+ variants covering: Variable, Literal, Property, TxIntrospection,
InputIntrospection, OutputIntrospection, AssetLookup, AssetAt, GroupFind, GroupProperty,
Sha256 streaming, Neg64, Le64ToScriptNum, EcMulScalarVerify, TweakVerify, etc.)

---

### Q6: `Cargo.toml` — Dependencies

```toml
[dependencies]
pest           = "2.7.8"          # PEG parser
pest_derive    = "2.7.8"          # proc-macro for grammar.pest
serde          = { version = "1.0.197", features = ["derive"] }
serde_json     = "1.0.114"
clap           = { version = "4.5.3", features = ["derive"] }
chrono         = "0.4.34"         # updatedAt timestamps
anyhow         = "1.0.89"
tracing        = "0.1.41"
tracing-subscriber = { version = "0.3.19", features = ["fmt","env-filter"] }
sha2           = "0.10.8"
hex            = "0.4.3"

wasm-bindgen          = { version = "0.2", optional = true }
console_error_panic_hook = { version = "0.1", optional = true }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
ratatui   = "0.29.0"             # TUI debugger
crossterm = "0.28.1"
secp256k1 = { version = "0.31.1", features = ["global-context"] }

[dev-dependencies]
tempfile   = "3.10.1"
assert_fs  = "1.1.1"
predicates = "3.1.0"

[features]
default = []
wasm = ["wasm-bindgen", "console_error_panic_hook"]
```

`secp256k1` is **native-only**. On WASM, `ExecutionEnv` uses a SHA256-based stub for
keypair derivation and signature production (deterministic but not real cryptography).

---

### Q7: `tests/` Directory — Test Files and Patterns

#### Full file list (50 files)

**Runtime unit tests:**
- `runtime_arithmetic_test.rs` — arithmetic opcodes
- `runtime_stack_test.rs` — stack manipulation
- `runtime_crypto_test.rs` — SHA256, checksig
- `runtime_opcode_surface_test.rs` — opcode coverage
- `runtime_binding_inference_test.rs` — default_bindings_for_program logic
- `runtime_introspection_test.rs` — tx introspection opcodes
- `runtime_extended_opcodes_test.rs` — streaming SHA, LE64, neg64, etc.
- `runtime_determinism_test.rs` — same inputs → same output

**Parity vector tests:**
- `runtime_parity_vectors_test.rs` — reads all `tests/parity_vectors/*.json`
- `runtime_external_parity_audit_test.rs` — broader audit
- `runtime_external_introspector_parity_test.rs` — reads `tests/parity_vectors_introspector/*.json`
- `runtime_external_introspector_delta_matrix_test.rs`

**CLI integration tests:**
- `runtime_cli_run_test.rs` — uses `env!("CARGO_BIN_EXE_arkadec")` + `std::process::Command`
- `runtime_cli_matrix_test.rs` — runs every function of every example JSON
- `runtime_cli_debug_boot_test.rs` — headless debugger boot

**E2E / artifact tests:**
- `runtime_artifact_htlc_test.rs` — library API on `examples/htlc.json`
- `runtime_examples_e2e_test.rs` — library API across all example JSONs
- `runtime_examples_dual_source_e2e_test.rs` — compares .ark source vs .json artifact
- `runtime_failure_classification_test.rs`

**Contract-specific tests (compile + ASM shape checks):**
- `htlc_test.rs`, `arkade_kitties_test.rs`, `bare_vtxo_test.rs`, `beacon_test.rs`,
  `controlled_mint_test.rs`, `epoch_limiter_test.rs`, `fee_adapter_test.rs`,
  `fuji_safe_test.rs`, `group_properties_test.rs`, `io_introspection_test.rs`,
  `new_opcodes_test.rs`, `token_vault_test.rs`, `tx_introspection_test.rs`,
  `threshold_oracle_test.rs`, `asset_introspection_test.rs`

#### Parity Vector JSON schema (`tests/parity_vectors/005_checksig_verify_success.json`)

```json
{
  "name": "checksig_verify_success",
  "asm": ["<pubkey>", "<sig>", "OP_CHECKSIGVERIFY", "OP_1"],
  "strict_placeholders": false,
  "bindings": {
    "pubkey": { "type": "pubkey_label",    "label": "alice" },
    "sig":    { "type": "signature_label", "label": "alice" }
  },
  "expected": {
    "kind": "script_true",
    "error_code": null,
    "top": { "type": "int", "value": 1 }
  }
}
```

**ValueSpec types** (defined in `tests/parity_support/mod.rs`):

| type | fields | semantics |
|------|--------|-----------|
| `int` | `value: i64` | `StackValue::Int` |
| `bool` | `value: bool` | `StackValue::Bool` |
| `bytes_hex` | `value: String` | hex-decoded bytes |
| `bytes_utf8` | `value: String` | UTF-8 bytes |
| `symbol` | `value: String` | `StackValue::Symbol` |
| `pubkey_label` | `label: String` | derives keypair from label, returns 33-byte compressed pubkey |
| `tx_group_txid` | `index: usize` | `TxContext.asset_groups[index].txid` |
| `tx_group_gidx` | `index: usize` | `TxContext.asset_groups[index].gidx` as Int |
| `signature_label` | `label: String`, `message_key?: String` | signs `tx_hash` (or `bindings[message_key]`) with label's key |

**`parity_support::materialize_bindings`** two-pass: first all non-signature specs,
then signature specs (so message_key bindings are already resolved).

#### Pattern used in `runtime_cli_run_test.rs`

```rust
fn cli_run_succeeds_for_htlc_claim_exit_variant() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run").arg("examples/htlc.json")
        .arg("--function").arg("claim")
        .arg("--variant").arg("false")
        .output().expect("failed to run cli");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("RESULT: true"));
}
```

---

## Architecture Map

```
.ark source
    │
    ▼
compiler::compile()  (src/compiler/mod.rs)
    │
    ▼
ContractJson  (src/models/mod.rs)  ──── JSON artifact on disk
    │
    ▼
runtime::load_program_from_contract()
    │  selects function by name+serverVariant, collects param_types
    ▼
LoadedProgram { asm: Vec<String>, param_types: ... }
    │
    ├─── default_bindings_for_program()  → HashMap<String, StackValue>
    │         deterministic keypairs, typed defaults, sig scanning
    │
    ▼
ExecutionEnv { bindings, strict_placeholders, tx_context: TxContext::sample() }
    │
    ▼
runtime::execute_program()  →  VMState::new(asm).run(&env)
    │
    ▼
VmRunResult { outcome: ScriptTrue|ScriptFalse|RuntimeError, telemetry, final_main_stack, ... }
```

**Entry points:**
- CLI: `src/main.rs` `run_command()` → exit codes 0/1/2
- WASM: `src/wasm.rs` `execute_contract_json()` / `execute_source()` → JSON string
- Library: `arkade_compiler::runtime::execute_program()` (used by tests)

---

## Key Files Summary

| File | Purpose | Key Entry Points |
|------|---------|-----------------|
| `src/main.rs` | CLI (arkadec) | `run_command()`, `compile_command()`, `debug_command()` |
| `src/wasm.rs` | WASM exports | `execute_contract_json()`, `execute_source()`, `compile()` |
| `src/runtime/env.rs` | Execution environment | `TxContext::sample()`, `ExecutionEnv::new()`, `stack_value_to_bytes()` |
| `src/runtime/mod.rs` | Runtime orchestration | `load_program_from_contract()`, `execute_program()`, `default_bindings_for_program()` |
| `src/models/mod.rs` | ABI + AST types | `ContractJson`, `AbiFunction`, `LoadedProgram` |
| `src/runtime/vm.rs` | Script VM | `VMState::new()`, `vm.run(&env)` → `VmRunResult` |
| `src/runtime/dispatcher.rs` | Opcode dispatch | called by VM per token |
| `tests/parity_support/mod.rs` | Test fixtures | `load_vectors()`, `materialize_bindings()`, `to_stack_value()` |

## Open Questions

- `src/runtime/vm.rs` and `src/runtime/dispatcher.rs` were not read — internal VM loop and opcode
  implementations are not mapped here but are not needed for the ABI/CLI/WASM surface.
- `src/lib.rs` not read — likely just re-exports of `compiler::compile`, `models::ContractJson`,
  `runtime::*`.
