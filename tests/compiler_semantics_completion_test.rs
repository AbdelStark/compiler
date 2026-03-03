use arkade_compiler::compile;
use arkade_compiler::runtime;
use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::VmOutcome;

#[test]
fn scenario_1_var_assign_updates_active_stack_value() {
    let code = r#"
        contract Reassign() {
            function run() {
                let x = 5;
                x = 10;
            }
        }
    "#;

    let artifact = compile(code).expect("contract should compile");
    let program = runtime::load_program_from_contract(&artifact, "run", true)
        .expect("program should load from compiled artifact");

    let mut env = ExecutionEnv::default();
    env.bindings = runtime::default_bindings_for_program(&program);

    let result = runtime::execute_program(&program, &env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
    assert_eq!(
        result.final_main_stack.last(),
        Some(&StackValue::Int(10)),
        "expected top-of-stack to reflect reassigned value, got {:?}",
        result.final_main_stack
    );
}

#[test]
fn scenario_2_array_index_literal_uses_flattened_binding_and_keeps_value_accessible() {
    let code = r#"
        contract ArrayAccess(int[] arr) {
            function read() {
                let selected = arr[0];
            }
        }
    "#;

    let artifact = compile(code).expect("contract should compile");
    let program = runtime::load_program_from_contract(&artifact, "read", true)
        .expect("program should load from compiled artifact");

    assert!(
        program.asm.iter().any(|op| op == "<arr_0>"),
        "expected flattened placeholder <arr_0>, got {:?}",
        program.asm
    );

    let mut env = ExecutionEnv::default();
    env.bindings = runtime::default_bindings_for_program(&program);
    env.bindings
        .insert("arr_0".to_string(), StackValue::Int(42));

    let result = runtime::execute_program(&program, &env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
    assert!(
        result.final_main_stack.contains(&StackValue::Int(42)),
        "expected indexed value 42 to be accessible, got {:?}",
        result.final_main_stack
    );
}

#[test]
fn scenario_3_function_call_statement_is_rejected_with_source_location() {
    let code = r#"
        contract UnsupportedCall() {
            function run() {
                helper();
            }
        }
    "#;

    let err = compile(code)
        .expect_err("function call statements must be rejected")
        .to_string();
    assert!(
        err.contains("line"),
        "expected error to include source location, got: {err}"
    );
    assert!(
        err.contains("helper()"),
        "expected error to include unsupported construct text, got: {err}"
    );
}

#[test]
fn dynamic_array_index_is_rejected_with_compile_error() {
    let code = r#"
        contract DynamicIndex(int[] arr, int idx) {
            function read() {
                let selected = arr[idx];
            }
        }
    "#;

    let err = compile(code)
        .expect_err("dynamic array indices must be rejected before codegen")
        .to_string();
    assert!(
        err.contains("arr[idx]"),
        "expected error to identify dynamic array index expression, got: {err}"
    );
}
