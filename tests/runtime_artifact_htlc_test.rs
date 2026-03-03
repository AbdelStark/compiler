use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::vm::VmOutcome;

#[test]
fn execute_htlc_claim_exit_variant() {
    let program =
        arkade_compiler::runtime::load_program_from_file("examples/htlc.json", "claim", false)
            .expect("program should load");

    let mut env = ExecutionEnv::default();
    env.bindings = arkade_compiler::runtime::default_bindings_for_program(&program);

    let result = arkade_compiler::runtime::execute_program(&program, &env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn missing_function_returns_error() {
    let err = arkade_compiler::runtime::load_program_from_file(
        "examples/htlc.json",
        "does_not_exist",
        false,
    )
    .expect_err("missing function should error");

    assert_eq!(
        err.code,
        arkade_compiler::runtime::error::RuntimeErrorCode::FunctionNotFound
    );
}
