use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::stack::Stack;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};

#[test]
fn arithmetic_opcodes_execute() {
    let script = vec!["OP_ADD64".to_string(), "OP_MUL64".to_string()];
    let initial_stack = Stack::with_main(
        vec![StackValue::Int(2), StackValue::Int(3), StackValue::Int(4)],
        1000,
    );

    let mut vm = VMState::with_stack(script, initial_stack);
    let result = vm.run(&ExecutionEnv::default());

    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
    assert_eq!(result.final_main_stack, vec![StackValue::Int(14)]);
}

#[test]
fn division_by_zero_is_runtime_error() {
    let script = vec!["OP_DIV64".to_string()];
    let initial_stack = Stack::with_main(vec![StackValue::Int(10), StackValue::Int(0)], 1000);

    let mut vm = VMState::with_stack(script, initial_stack);
    let result = vm.run(&ExecutionEnv::default());

    match result.outcome {
        VmOutcome::RuntimeError(err) => {
            assert_eq!(
                err.code,
                arkade_compiler::runtime::error::RuntimeErrorCode::DivisionByZero
            );
        }
        other => panic!("expected runtime error, got {:?}", other),
    }
}

#[test]
fn invalid_numeric_encoding_is_runtime_error() {
    let script = vec!["OP_ADD64".to_string()];
    let initial_stack = Stack::with_main(
        vec![StackValue::Symbol("abc".to_string()), StackValue::Int(3)],
        1000,
    );

    let mut vm = VMState::with_stack(script, initial_stack);
    let result = vm.run(&ExecutionEnv::default());

    match result.outcome {
        VmOutcome::RuntimeError(err) => {
            assert_eq!(
                err.code,
                arkade_compiler::runtime::error::RuntimeErrorCode::InvalidNumericEncoding
            );
        }
        other => panic!("expected runtime error, got {:?}", other),
    }
}
