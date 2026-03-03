use std::fs;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::vm::VmOutcome;
use arkade_compiler::ContractJson;

#[test]
fn execute_all_example_functions_without_runtime_errors() {
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
        let raw = fs::read_to_string(file).expect("example file should exist");
        let artifact: ContractJson = serde_json::from_str(&raw).expect("example json should parse");

        for function in &artifact.functions {
            let program = arkade_compiler::runtime::load_program_from_contract(
                &artifact,
                &function.name,
                function.server_variant,
            )
            .expect("program should load");

            let mut env = ExecutionEnv::default();
            env.bindings = arkade_compiler::runtime::default_bindings_for_program(&program);

            let result = arkade_compiler::runtime::execute_program(&program, &env);
            if let VmOutcome::RuntimeError(err) = result.outcome {
                failures.push(format!(
                    "{file}::{}(serverVariant={}) => runtime_error: {}",
                    function.name, function.server_variant, err
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "runtime errors in example execution:\n{}",
            failures.join("\n")
        );
    }
}
