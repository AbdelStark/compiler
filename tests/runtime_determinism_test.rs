use std::fs;
use std::process::Command;

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

            let env1 = ExecutionEnv {
                bindings: arkade_compiler::runtime::default_bindings_for_program(&program),
                ..ExecutionEnv::default()
            };
            let run1 = arkade_compiler::runtime::execute_program(&program, &env1);

            let env2 = ExecutionEnv {
                bindings: arkade_compiler::runtime::default_bindings_for_program(&program),
                ..ExecutionEnv::default()
            };
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

#[test]
fn context_file_execution_is_deterministic_for_identical_contract_context_and_bindings() {
    let temp = tempfile::tempdir().expect("tempdir should be created");
    let contract_path = temp.path().join("contract.json");
    let fixture_path = temp.path().join("fixture.json");

    fs::write(
        &contract_path,
        r#"{
  "contractName": "determinism_fixture_check",
  "constructorInputs": [],
  "functions": [
    {
      "name": "claim",
      "functionInputs": [],
      "serverVariant": false,
      "require": [],
      "asm": ["<counter>", "7", "OP_EQUALVERIFY", "OP_TXHASH", "abcd1234", "OP_EQUAL"]
    }
  ]
}"#,
    )
    .expect("contract fixture should be written");

    fs::write(
        &fixture_path,
        r#"{
  "tx_context": {
    "txid": "0xabcd1234"
  }
}"#,
    )
    .expect("context fixture should be written");

    let run_once = || {
        Command::new(env!("CARGO_BIN_EXE_arkadec"))
            .arg("run")
            .arg(&contract_path)
            .arg("--function")
            .arg("claim")
            .arg("--variant")
            .arg("false")
            .arg("--bind")
            .arg("counter=7")
            .arg("--context-file")
            .arg(&fixture_path)
            .arg("--output")
            .arg("json")
            .output()
            .expect("failed to run cli with context fixture")
    };

    let first = run_once();
    let second = run_once();

    assert_eq!(first.status.code(), Some(0));
    assert_eq!(second.status.code(), Some(0));

    let first_json: serde_json::Value =
        serde_json::from_slice(&first.stdout).expect("first run stdout should be valid json");
    let second_json: serde_json::Value =
        serde_json::from_slice(&second.stdout).expect("second run stdout should be valid json");

    assert_eq!(first_json, second_json);
    assert_eq!(first_json["result"]["outcome"], "script_true");
}
