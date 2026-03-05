use std::collections::HashMap;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::error::RuntimeErrorCode;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};
use arkade_compiler::runtime::LoadedProgram;

#[test]
fn strict_types_rejects_bool_to_i64_coercion() {
    let mut env = ExecutionEnv::default();
    env.strict_types = true;

    let script = vec!["true".to_string(), "OP_1".to_string(), "OP_ADD".to_string()];
    let mut vm = VMState::new(script);
    let result = vm.run(&env);

    match result.outcome {
        VmOutcome::RuntimeError(err) => {
            assert_eq!(err.code, RuntimeErrorCode::InvalidNumericEncoding)
        }
        other => panic!("expected strict type runtime error, got {other:?}"),
    }
}

#[test]
fn strict_bindings_requires_all_placeholders() {
    let program = LoadedProgram {
        contract_name: "StrictBindings".to_string(),
        function_name: "spend".to_string(),
        server_variant: false,
        asm: vec![
            "<missing>".to_string(),
            "OP_VERIFY".to_string(),
            "OP_1".to_string(),
        ],
        param_types: HashMap::new(),
    };

    let mut env = ExecutionEnv::default();
    env.strict_bindings = true;
    env.bindings
        .insert("present".to_string(), StackValue::Int(1));

    let result = arkade_compiler::runtime::execute_program(&program, &env);
    match result.outcome {
        VmOutcome::RuntimeError(err) => assert_eq!(err.code, RuntimeErrorCode::MissingBinding),
        other => panic!("expected missing binding runtime error, got {other:?}"),
    }
}
