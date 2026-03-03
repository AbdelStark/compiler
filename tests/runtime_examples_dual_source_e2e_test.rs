use std::fs;
use std::io::Write;
use std::path::PathBuf;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::vm::VmOutcome;
use arkade_compiler::ContractJson;
use tempfile::NamedTempFile;

fn top_level_example_files(extension: &str) -> Vec<PathBuf> {
    let mut files = fs::read_dir("examples")
        .expect("examples directory should exist")
        .map(|entry| entry.expect("failed reading examples entry").path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == extension)
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    files.sort();
    files
}

fn paired_json_ark_examples() -> Vec<(PathBuf, PathBuf)> {
    let json_files = top_level_example_files("json");
    let mut pairs = Vec::new();

    for json_path in json_files {
        let ark_path = json_path.with_extension("ark");
        if ark_path.is_file() {
            pairs.push((json_path, ark_path));
        }
    }

    pairs
}

fn execute_contract_all_functions(contract: &ContractJson, label: &str) -> Vec<String> {
    let mut failures = Vec::new();

    for function in &contract.functions {
        let program = match arkade_compiler::runtime::load_program_from_contract(
            contract,
            &function.name,
            function.server_variant,
        ) {
            Ok(program) => program,
            Err(err) => {
                failures.push(format!(
                    "{label}::{}(serverVariant={}) => load_program_error: {}",
                    function.name, function.server_variant, err
                ));
                continue;
            }
        };

        let mut env = ExecutionEnv::default();
        env.bindings = arkade_compiler::runtime::default_bindings_for_program(&program);

        let result = arkade_compiler::runtime::execute_program(&program, &env);
        if let VmOutcome::RuntimeError(err) = result.outcome {
            failures.push(format!(
                "{label}::{}(serverVariant={}) => runtime_error: {}",
                function.name, function.server_variant, err
            ));
        }
    }

    failures
}

#[test]
fn execute_all_top_level_json_examples_without_runtime_errors() {
    let mut failures = Vec::new();

    for path in top_level_example_files("json") {
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(err) => {
                failures.push(format!("{} => read_error: {err}", path.display()));
                continue;
            }
        };

        let contract: ContractJson = match serde_json::from_str(&raw) {
            Ok(contract) => contract,
            Err(err) => {
                failures.push(format!("{} => parse_error: {err}", path.display()));
                continue;
            }
        };

        failures.extend(execute_contract_all_functions(
            &contract,
            path.to_string_lossy().as_ref(),
        ));
    }

    if !failures.is_empty() {
        panic!(
            "runtime errors in top-level json example matrix:\n{}",
            failures.join("\n")
        );
    }
}

#[test]
fn execute_all_top_level_ark_examples_without_runtime_errors() {
    let mut failures = Vec::new();

    let pairs = paired_json_ark_examples();
    assert!(
        !pairs.is_empty(),
        "expected at least one top-level .json/.ark example pair"
    );

    for (_json_path, path) in pairs {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) => {
                failures.push(format!("{} => read_error: {err}", path.display()));
                continue;
            }
        };

        let contract = match arkade_compiler::compile(&source) {
            Ok(contract) => contract,
            Err(err) => {
                failures.push(format!("{} => compile_error: {err}", path.display()));
                continue;
            }
        };

        failures.extend(execute_contract_all_functions(
            &contract,
            path.to_string_lossy().as_ref(),
        ));
    }

    if !failures.is_empty() {
        panic!(
            "runtime errors in top-level ark source matrix:\n{}",
            failures.join("\n")
        );
    }
}

#[test]
fn execute_all_top_level_ark_examples_via_temp_json_artifacts_without_runtime_errors() {
    let mut failures = Vec::new();

    let pairs = paired_json_ark_examples();
    assert!(
        !pairs.is_empty(),
        "expected at least one top-level .json/.ark example pair"
    );

    for (_json_path, path) in pairs {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) => {
                failures.push(format!("{} => read_error: {err}", path.display()));
                continue;
            }
        };

        let contract = match arkade_compiler::compile(&source) {
            Ok(contract) => contract,
            Err(err) => {
                failures.push(format!("{} => compile_error: {err}", path.display()));
                continue;
            }
        };

        let mut temp = NamedTempFile::new().expect("failed creating temp artifact");
        let serialized =
            serde_json::to_string_pretty(&contract).expect("failed serializing compiled contract");
        temp.write_all(serialized.as_bytes())
            .expect("failed writing temp artifact");

        for function in &contract.functions {
            let program = match arkade_compiler::runtime::load_program_from_file(
                temp.path(),
                &function.name,
                function.server_variant,
            ) {
                Ok(program) => program,
                Err(err) => {
                    failures.push(format!(
                        "{} => load_program_error for {}(serverVariant={}): {}",
                        path.display(),
                        function.name,
                        function.server_variant,
                        err
                    ));
                    continue;
                }
            };

            let mut env = ExecutionEnv::default();
            env.bindings = arkade_compiler::runtime::default_bindings_for_program(&program);

            let result = arkade_compiler::runtime::execute_program(&program, &env);
            if let VmOutcome::RuntimeError(err) = result.outcome {
                failures.push(format!(
                    "{} => runtime_error for {}(serverVariant={}): {}",
                    path.display(),
                    function.name,
                    function.server_variant,
                    err
                ));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "runtime errors in ark source -> temp json artifact matrix:\n{}",
            failures.join("\n")
        );
    }
}
