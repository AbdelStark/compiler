use std::collections::BTreeSet;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::error::RuntimeErrorCode;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};

#[test]
fn max_steps_policy_is_enforced() {
    let mut env = ExecutionEnv::default();
    env.runtime_policy.max_steps = Some(1);

    let script = vec!["OP_1".to_string(), "OP_1".to_string(), "OP_ADD".to_string()];
    let mut vm = VMState::new(script);
    let result = vm.run(&env);

    match result.outcome {
        VmOutcome::RuntimeError(err) => assert_eq!(err.code, RuntimeErrorCode::PolicyViolation),
        other => panic!("expected policy runtime error, got {other:?}"),
    }
}

#[test]
fn allowed_opcode_policy_blocks_unlisted_opcode() {
    let mut env = ExecutionEnv::default();
    env.runtime_policy.allowed_opcodes = Some(
        ["OP_1".to_string()]
            .into_iter()
            .collect::<BTreeSet<String>>(),
    );

    let script = vec!["OP_1".to_string(), "OP_SHA256".to_string()];
    let mut vm = VMState::new(script);
    let result = vm.run(&env);

    match result.outcome {
        VmOutcome::RuntimeError(err) => assert_eq!(err.code, RuntimeErrorCode::PolicyViolation),
        other => panic!("expected policy runtime error, got {other:?}"),
    }
}
