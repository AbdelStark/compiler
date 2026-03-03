use std::fs;
use std::process::Command;

use arkade_compiler::ContractJson;

#[test]
fn cli_run_all_examples_returns_script_result_not_runtime_error() {
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
            let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
                .arg("run")
                .arg(file)
                .arg("--function")
                .arg(&function.name)
                .arg("--variant")
                .arg(function.server_variant.to_string())
                .output()
                .expect("failed to run arkadec");

            let code = output.status.code().unwrap_or(-1);
            if code == 2 {
                let stderr = String::from_utf8_lossy(&output.stderr);
                failures.push(format!(
                    "{file}::{}(serverVariant={}) => exit=2 stderr={}",
                    function.name,
                    function.server_variant,
                    stderr.trim()
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "CLI runtime errors encountered while executing matrix:\n{}",
            failures.join("\n")
        );
    }
}
