use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::stack::Stack;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};

#[test]
fn stack_manipulation_happy_path() {
    let script = vec![
        "OP_DUP".to_string(),
        "OP_DROP".to_string(),
        "OP_NIP".to_string(),
    ];
    let initial_stack = Stack::with_main(
        vec![StackValue::Int(1), StackValue::Int(2), StackValue::Int(3)],
        1000,
    );

    let mut vm = VMState::with_stack(script, initial_stack);
    let result = vm.run(&ExecutionEnv::default());

    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
    assert_eq!(
        result.final_main_stack,
        vec![StackValue::Int(1), StackValue::Int(3)]
    );
}

#[test]
fn stack_underflow_is_runtime_error() {
    let script = vec!["OP_DUP".to_string()];
    let mut vm = VMState::new(script);
    let result = vm.run(&ExecutionEnv::default());

    match result.outcome {
        VmOutcome::RuntimeError(err) => {
            assert_eq!(
                err.code,
                arkade_compiler::runtime::error::RuntimeErrorCode::StackUnderflow
            );
        }
        other => panic!("expected runtime error, got {:?}", other),
    }
}
