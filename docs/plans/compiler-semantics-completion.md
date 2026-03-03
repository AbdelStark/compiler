# Implementation Plan: Compiler Semantics Completion

**Unit ID**: `compiler-semantics-completion`
**Date**: 2026-03-03
**Category**: large
**Bounded Context**: contract-compilation

---

## 1. Work Type Assessment

**TDD applies.** All four gaps change observable behavior:

| Gap | Before | After | Classification |
|-----|--------|-------|----------------|
| VarAssign | Silent no-op (zero opcodes) | Emits value expression onto stack | Bug fix / feature |
| ArrayIndex | Emits sub-expressions but no lookup | Lowers `arr[lit]` to `<arr_lit>` or rejects dynamic | Feature |
| FunctionCallStmt | Silently dropped at parse time | Returns compile error with source location | Bug fix |
| Binding inference | Silent `Symbol→Int(0)` coercion | Warns on stderr before coercing | Feature |
| Introspection errors | `InvalidNumericEncoding` for all | `IndexOutOfBounds` / `AssetNotFound` | Bug fix |

Every change has new observable output (opcodes, error messages, stderr warnings, error codes) that can be tested before implementation.

---

## 2. Architecture Decisions

### 2.1 VarAssign: Simple Push (Option A from research)

**Decision**: Emit the value expression only (identical to `LetBinding`), NOT full stack-slot replacement (Option B).

**Rationale**:
- `LetBinding` already just pushes the value with `generate_expression_asm(value, asm)` (line 670)
- No stack-slot tracking infrastructure exists today
- The RFC says "stack-slot update opcodes" but Option A satisfies the domain invariant: "every grammar-accepted construct must either lower to valid opcodes or produce a precise compile-time rejection"
- A later unit can add `OP_ROLL`/`OP_SWAP` stack discipline if needed
- The acceptance criterion says: "produces a result consistent with x holding value 10" — pushing 10 onto the stack makes it the active value at that point

### 2.2 ArrayIndex: Pre-Generation Validation for Dynamic Indices

**Decision**: Handle literal indices inline in both `generate_expression_asm` and `emit_expression_asm`. For dynamic (non-literal) indices, add a **pre-generation AST validation pass** that runs before ASM generation, returning `Err(...)` from `compile()`.

**Rationale**:
- `generate_expression_asm` returns `()` (line 736) with 24 call sites
- `emit_expression_asm` returns `()` (line 1221) with 31 call sites
- Changing all signatures to `Result<(), String>` touches 55+ call sites — too large a refactor for this unit
- A validation pass before ASM generation is architecturally sound: validate, then generate
- The literal-index case (`arr[0]` → `<arr_0>`) emits a placeholder and needs no error path

### 2.3 FunctionCallStmt: Parser-Level Rejection (Option A)

**Decision**: Return `Err(...)` from `parse_function_body` when encountering `function_call_stmt`, using Pest's `pair.as_span().start_pos().line_col()` for source location.

**Rationale**: RFC says "reject early with precise parser errors." The parser already returns `Result<(), String>`, so `Err(...)` propagates cleanly through `parse()` → `compile()`.

### 2.4 Binding Inference Warning: `eprintln!` with Stderr Capture in Tests

**Decision**: Use `eprintln!` for warning output. Tests capture stderr using `gag` crate or `std::io` redirection. Do NOT change the function signature (API-stable).

**Rationale**:
- Changing `default_bindings_for_program` return type breaks the existing API and all callers
- `eprintln!` is the standard Rust idiom for diagnostic warnings
- For testing, we can use the `gag` crate (lightweight) or write a helper that returns warnings separately
- Alternative: add a `default_bindings_for_program_with_diagnostics` function that returns `(HashMap, Vec<String>)`, keeping the original as a thin wrapper. This is API-stable and testable.

**Final choice**: Add `default_bindings_for_program_with_diagnostics` → `(HashMap<String, StackValue>, Vec<String>)`. Original function becomes:
```rust
pub fn default_bindings_for_program(program: &LoadedProgram) -> HashMap<String, StackValue> {
    let (bindings, warnings) = default_bindings_for_program_with_diagnostics(program);
    for w in &warnings { eprintln!("{}", w); }
    bindings
}
```

This preserves API compatibility and enables deterministic test assertions without stderr capture.

### 2.5 Introspection Error Codes: New RuntimeErrorCode Variants

**Decision**: Add `IndexOutOfBounds` and `AssetNotFound` variants to `RuntimeErrorCode` enum.

**Rationale**: Using `InvalidNumericEncoding` for index/asset errors is semantically wrong and makes debugging harder. New variants are additive, non-breaking.

---

## 3. Step-by-Step Implementation (TDD Order)

### Phase 0: Pre-Flight
- [ ] `cargo test` — all existing tests green (baseline)
- [ ] `cargo clippy -- -D warnings` — clean

### Phase 1: Write All Failing Tests (RED)

Tests are written BEFORE implementation. Each test file corresponds to a BDD scenario.

#### Step 1.1: Create `tests/compiler_var_assign_test.rs`
```rust
// Scenario 1: var-reassign-compiles
#[test]
fn scenario_1_var_reassign_compiles() { ... }

#[test]
fn var_assign_emits_value_10_in_asm() { ... }

#[test]
fn var_assign_with_expression_compiles() { ... }
```

#### Step 1.2: Create `tests/compiler_array_index_test.rs`
```rust
// Scenario 2: array-index-emits-opcodes
#[test]
fn scenario_2_array_index_literal_emits_placeholder() { ... }

#[test]
fn array_index_dynamic_rejected_with_location() { ... }

#[test]
fn array_index_literal_1_emits_correct_suffix() { ... }
```

#### Step 1.3: Create `tests/compiler_function_call_rejection_test.rs`
```rust
// Scenario 3: function-call-statement-rejected-with-location
#[test]
fn scenario_3_function_call_rejected_with_line_number() { ... }

#[test]
fn function_call_error_message_includes_construct_text() { ... }
```

#### Step 1.4: Extend `tests/runtime_binding_inference_test.rs`
```rust
// Scenario 4: binding-inference-typed-default
#[test]
fn scenario_4_bytes_param_gets_bytes_default() { ... }

// Scenario 5: unknown-placeholder-emits-warning
#[test]
fn scenario_5_unknown_placeholder_warning_in_diagnostics() { ... }

#[test]
fn known_typed_placeholder_no_warning() { ... }
```

#### Step 1.5: Create `tests/runtime_introspection_error_codes_test.rs`
```rust
#[test]
fn out_of_bounds_group_index_returns_index_out_of_bounds() { ... }

#[test]
fn missing_asset_returns_asset_not_found() { ... }
```

### Phase 2: Implement (GREEN)

Work in ascending complexity order.

#### Step 2.1: FunctionCallStmt Rejection (parser, ~5 lines)

**File**: `src/parser/mod.rs`
**Location**: Lines 274–277

**Change**:
```rust
Rule::function_call_stmt => {
    let span = pair.as_span();
    let (line, _col) = span.start_pos().line_col();
    let text = pair.as_str().trim_end_matches(';').trim();
    Err(format!(
        "compile error at line {}: function call statement '{}' is not supported; \
         use require() or let bindings instead",
        line, text
    ))
}
```

**Verification**: `cargo test --test compiler_function_call_rejection_test`

#### Step 2.2: Binding Inference Warning (runtime, ~25 lines)

**File**: `src/runtime/mod.rs`
**Location**: Lines 86–179 (function `default_bindings_for_program`)

**Changes**:
1. Add `default_bindings_for_program_with_diagnostics` function
2. Refactor existing function to delegate to it
3. In the `Symbol` coercion loop (lines 172–176), iterate with `(name, value)` and collect warnings

```rust
pub fn default_bindings_for_program_with_diagnostics(
    program: &LoadedProgram,
) -> (HashMap<String, StackValue>, Vec<String>) {
    // ... same logic as current default_bindings_for_program ...
    let mut warnings = Vec::new();
    for (name, value) in bindings.iter_mut() {
        if matches!(value, StackValue::Symbol(_)) {
            warnings.push(format!(
                "warning: binding inference: unknown placeholder '{}' has no declared type, \
                 defaulting to Int(0)", name
            ));
            *value = StackValue::Int(0);
        }
    }
    (bindings, warnings)
}

pub fn default_bindings_for_program(program: &LoadedProgram) -> HashMap<String, StackValue> {
    let (bindings, warnings) = default_bindings_for_program_with_diagnostics(program);
    for w in &warnings { eprintln!("{}", w); }
    bindings
}
```

**Verification**: `cargo test --test runtime_binding_inference_test`

#### Step 2.3: ArrayIndex Literal Lowering (compiler, ~30 lines)

**File**: `src/compiler/mod.rs`
**Locations**: Lines 819–826 (`generate_expression_asm`) and Lines 1320–1327 (`emit_expression_asm`)

**Sub-step 2.3a**: Add AST validation function `validate_expressions_for_codegen`

```rust
fn validate_array_index_in_expression(expr: &Expression) -> Result<(), String> {
    match expr {
        Expression::ArrayIndex { array, index } => {
            match (array.as_ref(), index.as_ref()) {
                (Expression::Variable(_), Expression::Literal(_)) => Ok(()),
                _ => Err(format!(
                    "unsupported: dynamic array index access '{:?}[{:?}]'; \
                     only literal indices (arr[0], arr[1]) are supported",
                    array, index
                )),
            }
        }
        Expression::BinaryOp { left, right, .. } => {
            validate_array_index_in_expression(left)?;
            validate_array_index_in_expression(right)
        }
        // ... recursion for all compound expression variants
        _ => Ok(()),
    }
}
```

**Sub-step 2.3b**: Call validation from `compile()` before ASM generation (in `generate_function` or at the `compile()` level).

**Sub-step 2.3c**: Update `generate_expression_asm` ArrayIndex arm:
```rust
Expression::ArrayIndex { array, index } => {
    if let (Expression::Variable(arr_name), Expression::Literal(lit_idx)) =
        (array.as_ref(), index.as_ref())
    {
        asm.push(format!("<{}_{}>", arr_name, lit_idx));
    }
    // Dynamic indices caught by validation pass — unreachable in correct flow
}
```

**Sub-step 2.3d**: Same change in `emit_expression_asm`.

**Verification**: `cargo test --test compiler_array_index_test`

#### Step 2.4: VarAssign Emission (compiler, ~3 lines)

**File**: `src/compiler/mod.rs`
**Location**: Lines 672–674

**Change**:
```rust
Statement::VarAssign { name: _, value } => {
    // Emit the new value expression onto the stack
    // (same as LetBinding — stack discipline is caller's responsibility)
    generate_expression_asm(value, asm);
}
```

**Verification**: `cargo test --test compiler_var_assign_test`

#### Step 2.5: Introspection Error Codes (runtime, ~15 lines)

**File 1**: `src/runtime/error.rs`

Add new variants to `RuntimeErrorCode`:
```rust
pub enum RuntimeErrorCode {
    // ... existing variants ...
    IndexOutOfBounds,
    AssetNotFound,
}
```

**File 2**: `src/runtime/dispatcher.rs`

Replace three uses of `InvalidNumericEncoding` in `OP_INSPECTASSETGROUP` handler (lines 587–616):
- "asset group index N is out of bounds" → `RuntimeErrorCode::IndexOutOfBounds`
- "input/output index N is out of bounds" → `RuntimeErrorCode::IndexOutOfBounds`
- "group/io combination has no matching asset" → `RuntimeErrorCode::AssetNotFound`

**Verification**: `cargo test --test runtime_introspection_error_codes_test`

### Phase 3: Verify (REFACTOR)

- [ ] `cargo fmt`
- [ ] `cargo fmt --check` — clean
- [ ] `cargo clippy -- -D warnings` — clean
- [ ] `cargo test` — ALL tests green (old + new)
- [ ] Review each BDD scenario against acceptance criteria

---

## 4. Scenario-to-Test Mapping

| Scenario ID | BDD Scenario | Test File | Test Function(s) | Cargo Command |
|-------------|-------------|-----------|-------------------|---------------|
| `var-reassign-compiles` | Scenario 1: Variable reassignment lowers to stack-slot update | `tests/compiler_var_assign_test.rs` | `scenario_1_var_reassign_compiles`, `var_assign_emits_value_10_in_asm` | `cargo test --test compiler_var_assign_test` |
| `array-index-emits-opcodes` | Scenario 2: Array index access emits retrieval opcodes | `tests/compiler_array_index_test.rs` | `scenario_2_array_index_literal_emits_placeholder`, `array_index_dynamic_rejected_with_location` | `cargo test --test compiler_array_index_test` |
| `function-call-statement-rejected-with-location` | Scenario 3: Unsupported function-call emits located error | `tests/compiler_function_call_rejection_test.rs` | `scenario_3_function_call_rejected_with_line_number`, `function_call_error_message_includes_construct_text` | `cargo test --test compiler_function_call_rejection_test` |
| `binding-inference-typed-default` | Scenario 4: Binding inference respects declared param type | `tests/runtime_binding_inference_test.rs` | `scenario_4_bytes_param_gets_bytes_default` | `cargo test --test runtime_binding_inference_test` |
| `unknown-placeholder-emits-warning` | Scenario 5: Unknown placeholder emits diagnostic warning | `tests/runtime_binding_inference_test.rs` | `scenario_5_unknown_placeholder_warning_in_diagnostics`, `known_typed_placeholder_no_warning` | `cargo test --test runtime_binding_inference_test` |
| (introspection parity) | Richer error codes for introspection | `tests/runtime_introspection_error_codes_test.rs` | `out_of_bounds_group_index_returns_index_out_of_bounds`, `missing_asset_returns_asset_not_found` | `cargo test --test runtime_introspection_error_codes_test` |

---

## 5. Files to Create

| File | Purpose |
|------|---------|
| `tests/compiler_var_assign_test.rs` | Scenario 1 integration tests |
| `tests/compiler_array_index_test.rs` | Scenario 2 integration tests |
| `tests/compiler_function_call_rejection_test.rs` | Scenario 3 integration tests |
| `tests/runtime_introspection_error_codes_test.rs` | Introspection error code tests |

## 6. Files to Modify

| File | Changes | Zone |
|------|---------|------|
| `src/parser/mod.rs:274–277` | FunctionCallStmt → Err with line number | supervised |
| `src/runtime/mod.rs:86–179` | Add `_with_diagnostics` function, refactor original | autonomous |
| `src/compiler/mod.rs:672–674` | VarAssign → emit value expression | supervised |
| `src/compiler/mod.rs:819–826` | ArrayIndex literal → `<arr_N>` placeholder | supervised |
| `src/compiler/mod.rs:1320–1327` | ArrayIndex literal → `<arr_N>` placeholder (emit variant) | supervised |
| `src/compiler/mod.rs` (new fn) | Add `validate_array_index_in_expression` validation | supervised |
| `src/compiler/mod.rs` (compile fn) | Call validation before ASM generation | supervised |
| `src/runtime/error.rs` | Add `IndexOutOfBounds`, `AssetNotFound` variants | supervised |
| `src/runtime/dispatcher.rs:587–616` | Use new error codes | supervised |
| `tests/runtime_binding_inference_test.rs` | Add Scenario 4 + 5 tests | autonomous |

---

## 7. Invariant Validation

| Domain Invariant | Where Validated |
|-----------------|-----------------|
| Every grammar-accepted construct must lower to valid opcodes OR produce a precise compile-time rejection | VarAssign emits opcodes (Step 2.4); ArrayIndex emits placeholder or validation rejects (Step 2.3); FunctionCallStmt returns error (Step 2.1) |
| Default binding inference must never silently coerce a placeholder to a type that contradicts its declared parameter type | `apply_typed_default_binding` handles declared types correctly (verified in research); unknown placeholders now emit warning (Step 2.2) |

---

## 8. No-Slop Constraints

### Minimum Assertions Per Test

| Test | Min Assertions |
|------|----------------|
| `scenario_1_var_reassign_compiles` | 2: compilation succeeds + ASM contains "10" literal |
| `var_assign_emits_value_10_in_asm` | 3: compilation succeeds + ASM non-empty + "10" present |
| `scenario_2_array_index_literal_emits_placeholder` | 2: compilation succeeds + ASM contains `<arr_0>` |
| `array_index_dynamic_rejected_with_location` | 2: compilation fails + error contains "dynamic" or "non-literal" |
| `scenario_3_function_call_rejected_with_line_number` | 3: compilation fails + error contains "line" + error contains function name |
| `scenario_4_bytes_param_gets_bytes_default` | 2: binding exists + binding is `StackValue::Bytes(...)` |
| `scenario_5_unknown_placeholder_warning_in_diagnostics` | 3: warnings non-empty + warning contains placeholder name + binding still exists (Int(0)) |

### Forbidden Shortcuts

- **No `#[ignore]` on any new test** — all tests must be runnable
- **No `unwrap()` without expect message** in tests — use `expect("reason")`
- **No `assert!(true)` or `assert!(result.is_ok())` alone** — always check the value
- **No copying existing test code verbatim** — each test must test a distinct scenario
- **No `todo!()` or `unimplemented!()` in production code** — every path must be handled

### Verification Gates

Each implementation step must pass these gates before proceeding to the next:

1. `cargo build` — compiles
2. `cargo test --test <target>` — new test passes
3. `cargo test` — no regressions
4. `cargo clippy -- -D warnings` — no warnings

---

## 9. Risks and Mitigations

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| VarAssign simple push leaves stale values on stack | Scripts may have extra stack items at end | Medium | Existing contracts don't use VarAssign (it was a no-op); new tests verify behavior. Stack overflow caught by VM. Can be hardened later with OP_ROLL model. |
| Changing `generate_expression_asm` ArrayIndex arm without error path silently falls through | Dynamic index produces no output instead of error | High | Pre-generation validation pass catches dynamic indices before ASM generation. The in-expression handler only runs for validated (literal) indices. |
| FunctionCallStmt rejection breaks existing contracts that happen to have function calls | Compilation fails for previously-compiling contracts | Low | Current parser silently drops them, so no existing contract relies on function call behavior. Any contract with `foo();` was already broken (no-op). Rejection is strictly better. |
| `default_bindings_for_program_with_diagnostics` adds public API surface | API expands | Low | Function is useful for tooling. Mark `pub` with doc comment. Original function signature unchanged. |
| New RuntimeErrorCode variants break exhaustive matches | Compile errors in dispatcher or other match sites | Low | Only `Display` impl and error construction use `RuntimeErrorCode`; `Display` uses `{:?}` format which auto-includes new variants. No exhaustive match exists outside error creation. |
| Existing tests depend on `InvalidNumericEncoding` for group index errors | Test assertions break | Medium | Search existing tests for `InvalidNumericEncoding` assertions related to introspection before changing codes. |

---

## 10. Acceptance Criteria Verification

| # | Criterion | How Verified |
|---|-----------|-------------|
| 1 | `cargo build` and `cargo clippy -- -D warnings` pass clean | Phase 3 gate |
| 2 | `let x = 5; x = 10;` compiles; result consistent with x=10 | `scenario_1_var_reassign_compiles` asserts compilation + "10" in ASM |
| 3 | `arr[0]` compiles to opcodes; arr=[42,0,0] makes 42 accessible | `scenario_2_array_index_literal_emits_placeholder` asserts `<arr_0>` in ASM |
| 4 | Unsupported function-call returns error with source location | `scenario_3_function_call_rejected_with_line_number` asserts error + "line" |
| 5 | bytes-typed parameter gets bytes-typed binding default | `scenario_4_bytes_param_gets_bytes_default` asserts `StackValue::Bytes` |
| 6 | Unknown placeholder emits warning naming the placeholder | `scenario_5_unknown_placeholder_warning_in_diagnostics` asserts warning text |
| 7 | All pre-existing `cargo test` integration tests remain green | Phase 3 full test run |

---

## 11. Implementation Order Summary

```
Phase 1 (RED):
  1.1  tests/compiler_var_assign_test.rs           [Scenario 1]
  1.2  tests/compiler_array_index_test.rs           [Scenario 2]
  1.3  tests/compiler_function_call_rejection_test.rs [Scenario 3]
  1.4  tests/runtime_binding_inference_test.rs       [Scenario 4+5]
  1.5  tests/runtime_introspection_error_codes_test.rs [Introspection]

Phase 2 (GREEN) — ascending complexity:
  2.1  src/parser/mod.rs              — FunctionCallStmt rejection
  2.2  src/runtime/mod.rs             — Binding inference diagnostics
  2.3  src/compiler/mod.rs            — ArrayIndex validation + lowering
  2.4  src/compiler/mod.rs            — VarAssign emission
  2.5  src/runtime/error.rs + dispatcher.rs — Error codes

Phase 3 (REFACTOR):
  3.1  cargo fmt
  3.2  cargo clippy -- -D warnings
  3.3  cargo test (full suite)
```
