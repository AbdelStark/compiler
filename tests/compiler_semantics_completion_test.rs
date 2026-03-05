use arkade_compiler::compile;

#[test]
fn standalone_function_call_statement_is_explicit_noop() {
    let code = r#"
        contract RejectCalls(int n) {
            function spend() {
                helper();
                require(n >= 0);
            }
        }
    "#;

    let output = compile(code).expect("standalone function-call statement should compile");
    let function = output
        .functions
        .iter()
        .find(|f| f.name == "spend" && !f.server_variant)
        .expect("exit variant should exist");
    let asm_str = function.asm.join(" ");

    assert!(
        !asm_str.contains("helper("),
        "statement-level helper call should lower to explicit no-op, asm: {asm_str}"
    );
}

#[test]
fn array_length_access_compiles_to_static_length_placeholder() {
    let code = r#"
        contract ArrayLen(pubkey[] keys) {
            function spend() {
                let count = keys.length;
                require(count >= 1);
            }
        }
    "#;

    let output = compile(code).expect("array length contract should compile");
    let function = output
        .functions
        .iter()
        .find(|f| f.name == "spend" && !f.server_variant)
        .expect("exit variant should exist");

    assert!(
        !function.asm.iter().any(|token| token == "<count>"),
        "variable binding should be lowered, asm: {:?}",
        function.asm
    );
    assert!(
        function.asm.iter().any(|token| token == "3"),
        "array length should lower to default length, asm: {:?}",
        function.asm
    );
}

#[test]
fn var_assignment_is_lowered_without_placeholder_leak() {
    let code = r#"
        contract Reassign(int x) {
            function spend() {
                let y = x;
                y = 7;
                require(y == 7);
            }
        }
    "#;

    let output = compile(code).expect("reassignment contract should compile");
    let function = output
        .functions
        .iter()
        .find(|f| f.name == "spend" && !f.server_variant)
        .expect("exit variant should exist");

    assert!(
        !function.asm.iter().any(|token| token == "<y>"),
        "reassignment variable should be lowered, asm: {:?}",
        function.asm
    );
}
