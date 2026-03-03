use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::error::RuntimeErrorCode;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};

#[test]
fn verify_false_is_script_failure_not_runtime_error() {
    let script = vec!["OP_0".to_string(), "OP_VERIFY".to_string()];
    let mut vm = VMState::new(script);
    let result = vm.run(&ExecutionEnv::default());
    assert_eq!(result.outcome, VmOutcome::ScriptFalse);
}

#[test]
fn unknown_opcode_is_runtime_error() {
    let script = vec!["OP_DOES_NOT_EXIST".to_string()];
    let mut vm = VMState::new(script);
    let result = vm.run(&ExecutionEnv::default());
    match result.outcome {
        VmOutcome::RuntimeError(err) => assert_eq!(err.code, RuntimeErrorCode::UnknownOpcode),
        other => panic!("expected runtime error, got {other:?}"),
    }
}

#[test]
fn missing_placeholder_in_strict_mode_is_runtime_error() {
    let mut env = ExecutionEnv::default();
    env.strict_placeholders = true;

    let script = vec!["<missing>".to_string()];
    let mut vm = VMState::new(script);
    let result = vm.run(&env);

    match result.outcome {
        VmOutcome::RuntimeError(err) => assert_eq!(err.code, RuntimeErrorCode::MissingBinding),
        other => panic!("expected runtime error, got {other:?}"),
    }
}

#[test]
fn unbalanced_conditional_is_runtime_error() {
    let script = vec!["OP_0".to_string(), "OP_IF".to_string()];
    let mut vm = VMState::new(script);
    let result = vm.run(&ExecutionEnv::default());

    match result.outcome {
        VmOutcome::RuntimeError(err) => {
            assert_eq!(err.code, RuntimeErrorCode::UnbalancedConditional)
        }
        other => panic!("expected runtime error, got {other:?}"),
    }
}
