# Research: Compiler Semantics Completion

**Unit ID**: `compiler-semantics-completion`
**Date**: 2026-03-03
**RFC Reference**: `docs/rfc/001_advanced_runtime_features.md` §3, §3.1, §3.2, §3.3
**Bounded Context**: `contract-compilation`

---

## 1. Overview

This document maps all code paths required to close four compiler semantic gaps identified in RFC 001 §3 ("Compiler/Runtime Semantics Completion"):

1. **VarAssign** — variable reassignment is parsed but silently no-op in compiler
2. **Array features** — `array_index_access` and `array_literal` are stubbed
3. **FunctionCall statement** — silently dropped at parse time
4. **Binding inference** — unknown placeholders silently coerced to `Int(0)`

---

## 2. Domain Model

### Aggregates / Value Objects

| Entity | File | Description |
|--------|------|-------------|
| `Statement` enum | `src/models/mod.rs:115–135` | AST statement node; includes `VarAssign`, `LetBinding`, `IfElse`, `ForIn`, `Require` |
| `Expression` enum | `src/models/mod.rs:197–320` | AST expression node; includes `ArrayIndex`, `ArrayLength`, `Variable`, `Literal` |
| `LoadedProgram` | `src/runtime/mod.rs:24–30` | Runtime artifact; holds `asm: Vec<String>` and `param_types: HashMap<String,String>` |
| `StackValue` | `src/runtime/value.rs:4–9` | Runtime value (`Int`, `Bool`, `Bytes`, `Symbol`) |
| `ExecutionEnv` | `src/runtime/env.rs:198–224` | Runtime context; holds `bindings`, `strict_placeholders` |

### Key Constants

- `DEFAULT_ARRAY_LENGTH = 3` (`src/compiler/mod.rs:298`) — used to flatten array params and unroll loops

### Invariants (from Domain Context)

1. Every grammar-accepted construct must lower to valid opcodes **or** produce a precise compile-time rejection — silent no-ops forbidden
2. Default binding inference must never silently coerce a placeholder to a type contradicting its declared parameter type

---

## 3. Gap 1 — VarAssign (Variable Reassignment)

### Current State

**Grammar** (`src/parser/grammar.pest:82–84`):
```
var_assign = {
    identifier ~ "=" ~ !("=") ~ general_expression ~ ";"
}
```

**Parser** (`src/parser/mod.rs:205–218`):
```rust
Rule::var_assign => {
    // ... correctly extracts name + value ...
    func.statements.push(Statement::VarAssign { name, value });
    Ok(())
}
```

**Compiler** (`src/compiler/mod.rs:672–674`):
```rust
Statement::VarAssign { name: _, value: _ } => {
    // TODO: Implement variable reassignment
}
```

**Silent no-op** — VarAssign produces zero opcodes.

### What Works Today

- `statement_uses_introspection` correctly handles `VarAssign` (line 55–58)
- `substitute_statement` in loop unrolling correctly handles `VarAssign` (lines 1803–1806)
- `collect_asset_ids_from_statement` correctly handles `VarAssign` (line 268–270)

### Required Change

Bitcoin Script is stack-based with no mutable variables. The design choice is:

**Option A (minimal / safe)**: Emit the value expression — same as `LetBinding`. This pushes the new value onto the stack. The old value is **not removed** — caller is responsible for stack discipline.

**Option B (stack-slot model)**: Track the stack depth offset of each named variable, and emit `OP_ROLL` / `OP_SWAP` to update the value in-place. Requires a `slot_map: HashMap<String, usize>` passed through recursive generation.

Per RFC 001 §3.1: "Implement VarAssign lowering/slot model." Option B is the RFC intent, but Option A is a minimal viable step that satisfies the invariant (produce opcodes, not silence).

**Affected function**: `generate_asm_from_statements_recursive` at `src/compiler/mod.rs:591–676`

### Scenario 1 Test Path

```
.ark: "let x = 5; x = 10;"
→ parse() → [Statement::LetBinding{name:"x", value:Literal("5")},
              Statement::VarAssign{name:"x", value:Literal("10")}]
→ compile() → generate_asm_from_statements_recursive()
→ must emit opcodes for both statements
```

**New test file**: `tests/compiler_var_assign_test.rs`

---

## 4. Gap 2 — Array Index Access and Array Literal

### Grammar

**Array index access** (`grammar.pest:154–156`):
```
array_index_access = {
    identifier ~ "[" ~ (identifier | number_literal) ~ "]"
}
```

**Array literal** (`grammar.pest:452–454`):
```
array_literal = {
    "[" ~ complex_expression ~ ("," ~ complex_expression)* ~ "]"
}
```

### Parser Mapping

- `array_index_access` → `Expression::ArrayIndex { array: Box<Expression>, index: Box<Expression> }` (via `parse_general_expression`)
- `array_literal` in `complex_expression` → `Expression::Property(array_literal_str)` (line 614–620 — treated as raw string, not a structured AST node)

### Compiler Current State

**In `generate_expression_asm`** (`src/compiler/mod.rs:819–823`):
```rust
Expression::ArrayIndex { array, index } => {
    // TODO: Implement array indexing in Commit 6
    generate_expression_asm(array, asm);
    generate_expression_asm(index, asm);
}
```

**In `emit_expression_asm`** (`src/compiler/mod.rs:1320–1327`):
```rust
Expression::ArrayIndex { array, index } => {
    // TODO: Implement array indexing in Commit 6
    emit_expression_asm(array, asm);
    emit_expression_asm(index, asm);
}
```

Both emit `array` + `index` expressions but **do not** perform the actual lookup.

**`ArrayLength`** produces nothing at all (both locations empty).

### Arkade Array Model

Arkade compiles array access **at compile-time** by flattening:
- Constructor param `pubkey[] keys` → ABI params `keys_0`, `keys_1`, `keys_2`
- Loop-body `arr[i]` where `i` is the loop index → `Variable("arr_k")` via `substitute_expression`

**Key insight**: Array indexing with a **literal index** should lower to a flat variable reference:
- `arr[0]` → `<arr_0>`
- `arr[1]` → `<arr_1>`
- `arr[2]` → `<arr_2>`

Array indexing with a **non-literal, non-loop-variable index** (e.g., `arr[x]` where `x` is a runtime variable) is genuinely unsupported by the stack model and should produce a compile-time rejection error with the construct name.

### Required Change

In `generate_expression_asm` and `emit_expression_asm`:

```rust
Expression::ArrayIndex { array, index } => {
    match (array.as_ref(), index.as_ref()) {
        (Expression::Variable(arr_name), Expression::Literal(lit_idx)) => {
            // Lower arr[literal] → <arr_literal>
            asm.push(format!("<{}_{}>", arr_name, lit_idx));
        }
        _ => {
            // Runtime-dynamic index: compile-time rejection
            return Err(format!(
                "unsupported: array index access with non-literal index at '{:?}[{:?}]'",
                array, index
            ));
        }
    }
}
```

*Note*: Both `generate_expression_asm` and `emit_expression_asm` would need this — or they should be consolidated.

For `array_literal`: The current grammar handles array literals only in `complex_expression` context (wrapped as `Property`). No `Expression::ArrayLiteral` AST node exists. Adding a push-sequence lowering for array literals would require adding `Expression::ArrayLiteral(Vec<Expression>)` to `models/mod.rs`, updating the parser, and adding compiler emission. This is an **additive model change** requiring coordination across parser + models + compiler.

### Scenario 2 Test Path

```
.ark: arr parameter bound to [42, 0, 0], access arr[0]
→ compile() → Expression::ArrayIndex{Variable("arr"), Literal("0")}
→ emit → "<arr_0>"
→ runtime: bind arr_0=42, execute → 42 on stack
```

**Affected test**: `tests/compiler_array_index_test.rs`

---

## 5. Gap 3 — FunctionCall Statement

### Grammar (`grammar.pest:92–94`):
```
function_call_stmt = {
    identifier ~ "(" ~ (complex_expression ~ ("," ~ complex_expression)*)? ~ ")" ~ ";"
}
```

### Parser Current State (`src/parser/mod.rs:274–277`):
```rust
Rule::function_call_stmt => {
    // Function calls to internal helpers — not yet fully supported
    Ok(())
}
```

**Silent drop** — `function_call_stmt` pushes no statement to `func.statements`. The statement completely vanishes.

### What "Source Location" Means

The `Pair<Rule>` from Pest carries span information. `pair.as_str()` gives the matched text. `pair.as_span().start_pos().line_col()` gives `(line, col)`. This can be used to construct an error message like:

```
compile error at line 5: function call statement 'foo(bar)' is not supported; use require() or let bindings
```

### Two Options

**Option A (Compile-time rejection)**: Return `Err(...)` from `parse_function_body` when `function_call_stmt` is encountered, with the Pest span's line number.

**Option B (New AST variant + compiler rejection)**: Add `Statement::FunctionCallStmt { name: String, args: Vec<Expression>, line: usize }` to `models/mod.rs`, parse it properly, and then reject in the compiler with a helpful message.

Per RFC: "reject early with precise parser errors" — **Option A** is the RFC intent.

### Required Change

**`src/parser/mod.rs`** in `parse_function_body`:
```rust
Rule::function_call_stmt => {
    let span = pair.as_span();
    let (line, _col) = span.start_pos().line_col();
    let text = pair.as_str().to_string();
    Err(format!(
        "compile error at line {line}: function call statement '{text}' is not supported; \
         function calls must use require() or let bindings"
    ))
}
```

This makes the compiler return `Err(format!("Parse error: ..."))` from `compile()`.

### Scenario 3 Test Path

```
.ark: source with "foo(bar);"
→ parse() → Err("compile error at line N: function call statement 'foo(bar);' is not supported")
→ compile() → Err("Parse error: compile error at line N: ...")
```

**Affected test**: `tests/compiler_function_call_rejection_test.rs`

---

## 6. Gap 4 — Default Binding Inference

### Current State (`src/runtime/mod.rs:86–179`)

The function `default_bindings_for_program` scans ASM placeholders and resolves them:

1. Checks `program.param_types` → `apply_typed_default_binding` for known types ✓ VERIFIED
2. Special-cases: `preimage`, `hash`, `_txid`, `_gidx` suffixes ✓ VERIFIED
3. `is_probable_numeric` → `Int(0)` ✓ VERIFIED
4. Falls through to `Symbol(name)` for unknown placeholders
5. **Line 172–176**: ALL `Symbol` values are **silently coerced to `Int(0)`** — this is the "silent coercion" violation

```rust
for value in bindings.values_mut() {
    if matches!(value, StackValue::Symbol(_)) {
        *value = StackValue::Int(0);  // ← SILENT COERCION, no warning
    }
}
```

### `apply_typed_default_binding` (`src/runtime/mod.rs:230–272`)

Handles: `pubkey`, `signature`, `bytes32`, `bytes`, `int`/`value`, `bool` — returns `true`
Unknown type → returns `false`, falls through to later logic

### Scenario 4 — Typed Default Must Respect Declared Type

Given param declared as `bytes`:
- `apply_typed_default_binding(name, "bytes", ...)` → inserts `StackValue::Bytes(Vec::new())` ✓ VERIFIED

This **already works** for known types via `param_types`. The test must verify this works for `bytes` type specifically (currently the test only covers `pubkey`, `signature`, `refundTime`).

### Scenario 5 — Unknown Placeholder Emits Warning

Required change in the Symbol coercion loop:
```rust
for (name, value) in bindings.iter_mut() {
    if matches!(value, StackValue::Symbol(_)) {
        eprintln!(
            "warning: binding inference: unknown placeholder '{}' has no declared type, \
             defaulting to Int(0)",
            name
        );
        *value = StackValue::Int(0);
    }
}
```

**Constraint**: Test must capture stderr to verify the warning (use `std::io::Write` redirection or `assert_stderr` patterns). Alternatively, the function could return a `Vec<String>` of warnings alongside the bindings.

### Introspection Diagnostics (§3.3)

**Current state** (`src/runtime/dispatcher.rs:587–616`): Out-of-bounds errors use `RuntimeErrorCode::InvalidNumericEncoding` — imprecise for missing-index vs missing-asset.

**Required**: Add new error codes or richer messages:
- Missing index → `RuntimeErrorCode::IndexOutOfBounds` (new variant) or structured message
- Missing asset → `RuntimeErrorCode::AssetNotFound` (new variant)

These would require adding variants to `RuntimeErrorCode` in `src/runtime/error.rs`.

---

## 7. File-by-File Change Map

| File | Change | Scope |
|------|--------|-------|
| `src/compiler/mod.rs:672` | VarAssign: emit value expression (or slot model) | supervised |
| `src/compiler/mod.rs:819–823` | ArrayIndex: lower `arr[lit]` to `<arr_lit>`, reject dynamic | supervised |
| `src/compiler/mod.rs:1320–1324` | ArrayIndex (duplicate in emit_expression_asm): same fix | supervised |
| `src/compiler/mod.rs:824–826` | ArrayLength: either implement or reject | supervised |
| `src/parser/mod.rs:274–277` | FunctionCallStmt: return Err with line number | supervised |
| `src/runtime/mod.rs:172–176` | Unknown placeholder: emit eprintln! warning before Int(0) | autonomous |
| `src/runtime/error.rs` | Add `IndexOutOfBounds`, `AssetNotFound` error codes | supervised |
| `src/runtime/dispatcher.rs:587–616` | Use new error codes for missing-index/asset | supervised |

---

## 8. Test File Targets

| Test File | Scenario | Coverage |
|-----------|----------|----------|
| `tests/compiler_var_assign_test.rs` | Scenario 1 | VarAssign compiles without error + opcodes emitted |
| `tests/compiler_array_index_test.rs` | Scenario 2 | `arr[0]` lowers to `<arr_0>`, runtime makes value 42 available |
| `tests/compiler_function_call_rejection_test.rs` | Scenario 3 | `foo(bar);` returns Err with line number in message |
| `tests/runtime_binding_inference_test.rs` | Scenario 4 + 5 | bytes-typed param binding, warning for unknown placeholder |

### Existing Test to Extend

`tests/runtime_binding_inference_test.rs` already tests pubkey/signature/refundTime. New assertions needed:
- `bytes`-typed param returns `StackValue::Bytes(Vec::new())`
- Unknown placeholder produces stderr warning (check via output capture or warning list return)

### Integration Test Pattern (from existing tests)

```rust
// tests/runtime_arithmetic_test.rs pattern:
use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};

#[test]
fn var_assign_emits_opcodes() {
    let src = r#"contract Test() {
        function run() {
            let x = 5;
            x = 10;
            require(x >= 10);
        }
    }"#;
    let compiled = arkade_compiler::compile(src).expect("should compile");
    // find the run function and verify asm contains something for x = 10
    let func = compiled.functions.iter().find(|f| f.name == "run" && f.server_variant).unwrap();
    assert!(!func.asm.is_empty(), "asm should not be empty");
    // The literal "10" should appear in the asm
    assert!(func.asm.iter().any(|op| op == "10"), "10 must appear in asm");
}
```

---

## 9. Dependency Analysis

### VarAssign Depends On

- Understanding of stack-slot model vs. simple push
- Decision: does VarAssign also need `OP_DROP` of old value? Only if LetBinding pushed it before.
- In practice: if `let x = 5` pushed `5` and `x = 10` needs to replace it, the script must manage the stack. For simple top-of-stack reassignment, just emit new value.

### ArrayIndex Depends On

- Array parameter flattening already done in `decompose_constructor_params` and `generate_function`
- `param_types` in `LoadedProgram` maps `arr_0`, `arr_1`, `arr_2` to their base types
- Literal-index case is straightforward; dynamic-index case needs early rejection

### FunctionCallStmt Depends On

- Pest's `Pair::as_span().start_pos().line_col()` API — check pest crate version 2.8.x
- Changing from `Ok(())` to `Err(...)` affects the `parse_function_body` → `parse_function` → `parse_contract` → `parse()` → `compile()` error chain (all return `Result`)

### Binding Inference Depends On

- `eprintln!` is sufficient for warning (stderr)
- If test must assert warning content, needs stderr capture or alternative design (return `(HashMap, Vec<String>)`)
- `apply_typed_default_binding` already correct for declared types

---

## 10. Open Questions

1. **VarAssign slot model**: Should VarAssign emit `OP_DROP` + new value (replace top-of-stack), or just push new value (leaving old value below)? The RFC says "stack-slot update opcodes" suggesting proper replacement. Need decision on OP_ROLL approach.

2. **ArrayIndex dynamic rejection**: Should dynamic array index (`arr[x]` where x is not literal) be a **parse-time** error or **compile-time** error? Currently the AST node is created — rejection in compiler is simpler. Parser rejection requires grammar change.

3. **ArrayLiteral AST node**: Should `array_literal` in expressions get a proper `Expression::ArrayLiteral(Vec<Expression>)` AST node, or remain as `Property` string? Adding the AST node is cleaner but touches `models/mod.rs` (supervised zone) and parser (supervised zone).

4. **Warning mechanism for binding inference**: Should `default_bindings_for_program` use `eprintln!` directly (hard to test), or return a `(HashMap<String,StackValue>, Vec<String>)` tuple of (bindings, warnings)? The latter enables deterministic test assertions but breaks the existing API.

5. **FunctionCallStmt rejection**: The current grammar accepts `function_call_stmt` and the parser reaches `parse_function_body`. Can we get line information from the Pest span at that point? Answer: Yes, `pair.as_span().start_pos().line_col()` is available in `parse_function_body` since `pair: Pair<Rule>` is passed in.

6. **Introspection error codes**: Should `IndexOutOfBounds` and `AssetNotFound` be added to `RuntimeErrorCode` in `error.rs`, or should the existing `InvalidNumericEncoding` be used with richer messages? Adding new variants is cleaner but touches a supervised file.

---

## 11. RFC Section References

- **§3.1** (Compiler TODO closure): VarAssign slot model, array features, function-call rejection — this unit's core
- **§3.2** (Type-aware binding inference): `default_bindings_for_program` improvements — warn on unknown placeholders
- **§3.3** (Introspection parity): Richer missing-index and missing-asset diagnostics

---

## 12. Quick-Start for Implementation

```bash
# Build and run tests
cargo build
cargo test

# Run specific test files
cargo test --test compiler_var_assign_test
cargo test --test compiler_array_index_test
cargo test --test compiler_function_call_rejection_test
cargo test --test runtime_binding_inference_test

# Format check (required by CI)
cargo fmt --check
```

Key mutation points in order of complexity (low → high):

1. **FunctionCallStmt rejection** (3 lines changed, high-value): `src/parser/mod.rs:274`
2. **Binding inference warning** (3 lines changed): `src/runtime/mod.rs:172–176`
3. **ArrayIndex literal lowering** (10 lines, 2 locations): `src/compiler/mod.rs:819–823`, `1320–1324`
4. **VarAssign slot model** (most complex, requires design decision): `src/compiler/mod.rs:672–674`
5. **Introspection error codes** (touches error.rs supervised + dispatcher.rs supervised): `src/runtime/error.rs` + `src/runtime/dispatcher.rs`
