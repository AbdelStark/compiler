use std::fs;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::vm::VmOutcome;
use arkade_compiler::ContractJson;

#[test]
fn example_matrix_is_deterministic_and_runtime_safe() {
    let files = vec![
        "examples/arkade_kitties.json",
        "examples/fuji_safe.json",
        "examples/htlc.json",
        "examples/nft_mint.json",
        "examples/non_interactive_swap.json",
        "examples/payment_auth.json",
        "examples/single_sig.json",
    ];

    let mut failures: Vec<String> = Vec::new();

    for file in files {
        let raw = fs::read_to_string(file).expect("example should exist");
        let artifact: ContractJson = serde_json::from_str(&raw).expect("example should parse");

        for function in &artifact.functions {
            let program = arkade_compiler::runtime::load_program_from_contract(
                &artifact,
                &function.name,
                function.server_variant,
            )
            .expect("program should load");

            let mut env1 = ExecutionEnv::default();
            env1.bindings = arkade_compiler::runtime::default_bindings_for_program(&program);
            let run1 = arkade_compiler::runtime::execute_program(&program, &env1);

            let mut env2 = ExecutionEnv::default();
            env2.bindings = arkade_compiler::runtime::default_bindings_for_program(&program);
            let run2 = arkade_compiler::runtime::execute_program(&program, &env2);

            if let VmOutcome::RuntimeError(err) = &run1.outcome {
                failures.push(format!(
                    "{file}::{}(serverVariant={}) runtime_error: {err}",
                    function.name, function.server_variant
                ));
                continue;
            }

            if run1.outcome != run2.outcome
                || run1.final_main_stack != run2.final_main_stack
                || run1.final_alt_stack != run2.final_alt_stack
                || run1.telemetry.len() != run2.telemetry.len()
            {
                failures.push(format!(
                    "{file}::{}(serverVariant={}) is non-deterministic",
                    function.name, function.server_variant
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!("determinism failures:\n{}", failures.join("\n"));
    }
}
