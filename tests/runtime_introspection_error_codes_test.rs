use std::collections::HashMap;

use arkade_compiler::runtime;
use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::error::RuntimeErrorCode;
use arkade_compiler::runtime::vm::VmOutcome;
use arkade_compiler::runtime::LoadedProgram;

#[test]
fn out_of_bounds_group_index_returns_index_out_of_bounds() {
    let program = LoadedProgram {
        contract_name: "Diagnostics".to_string(),
        function_name: "run".to_string(),
        server_variant: true,
        asm: vec![
            "99".to_string(),
            "0".to_string(),
            "0".to_string(),
            "OP_INSPECTASSETGROUP".to_string(),
        ],
        param_types: HashMap::new(),
    };

    let env = ExecutionEnv::default();
    let result = runtime::execute_program(&program, &env);

    match result.outcome {
        VmOutcome::RuntimeError(err) => {
            assert_eq!(err.code, RuntimeErrorCode::IndexOutOfBounds);
            assert!(
                err.message
                    .contains("asset group index 99 is out of bounds"),
                "unexpected message: {}",
                err.message
            );
        }
        other => panic!("expected runtime error, got {other:?}"),
    }
}

#[test]
fn missing_asset_returns_asset_not_found() {
    let program = LoadedProgram {
        contract_name: "Diagnostics".to_string(),
        function_name: "run".to_string(),
        server_variant: true,
        asm: vec![
            "0".to_string(),
            "0".to_string(),
            "0".to_string(),
            "OP_INSPECTASSETGROUP".to_string(),
        ],
        param_types: HashMap::new(),
    };

    let mut env = ExecutionEnv::default();
    env.tx_context.inputs[0].assets.clear();

    let result = runtime::execute_program(&program, &env);

    match result.outcome {
        VmOutcome::RuntimeError(err) => {
            assert_eq!(err.code, RuntimeErrorCode::AssetNotFound);
            assert!(
                err.message
                    .contains("group/io combination has no matching asset"),
                "unexpected message: {}",
                err.message
            );
        }
        other => panic!("expected runtime error, got {other:?}"),
    }
}
