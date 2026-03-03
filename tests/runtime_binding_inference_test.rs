use std::collections::HashMap;

use arkade_compiler::compile;
use arkade_compiler::runtime::env::stack_value_to_bytes;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::LoadedProgram;

#[test]
fn default_bindings_use_artifact_abi_types() {
    let program =
        arkade_compiler::runtime::load_program_from_file("examples/htlc.json", "claim", true)
            .expect("program should load");

    let bindings = arkade_compiler::runtime::default_bindings_for_program(&program);

    for key in ["receiver", "SERVER_KEY"] {
        let value = bindings.get(key).unwrap_or_else(|| panic!("missing {key}"));
        match value {
            StackValue::Bytes(bytes) => {
                assert!(
                    bytes.len() == 33 || bytes.len() == 65,
                    "{key} bytes len invalid"
                );
            }
            other => panic!("{key} should be bytes pubkey, got {other:?}"),
        }
    }

    for key in ["receiverSig", "serverSig"] {
        let value = bindings.get(key).unwrap_or_else(|| panic!("missing {key}"));
        let bytes = stack_value_to_bytes(value);
        assert!(bytes.len() >= 64, "{key} signature should be present");
    }

    let refund =
        arkade_compiler::runtime::load_program_from_file("examples/htlc.json", "refund", false)
            .expect("program should load");
    let refund_bindings = arkade_compiler::runtime::default_bindings_for_program(&refund);

    let refund_time = refund_bindings
        .get("refundTime")
        .expect("missing refundTime default");
    assert_eq!(refund_time, &StackValue::Int(0));
}

#[test]
fn scenario_4_binding_inference_typed_bytes_defaults_to_empty_bytes() {
    let code = r#"
        contract TypedBytes(bytes payload) {
            function run() {
                let copy = payload;
            }
        }
    "#;

    let artifact = compile(code).expect("contract should compile");
    let program = arkade_compiler::runtime::load_program_from_contract(&artifact, "run", true)
        .expect("program should load");

    let (bindings, warnings) =
        arkade_compiler::runtime::default_bindings_for_program_with_diagnostics(&program);

    assert_eq!(
        bindings.get("payload"),
        Some(&StackValue::Bytes(Vec::new())),
        "bytes-typed placeholder should default to empty bytes"
    );
    assert!(
        warnings.iter().all(|warning| !warning.contains("payload")),
        "typed placeholders should not emit unknown-placeholder warnings: {warnings:?}"
    );
}

#[test]
fn scenario_5_unknown_placeholder_emits_warning_diagnostics() {
    let program = LoadedProgram {
        contract_name: "Warnings".to_string(),
        function_name: "run".to_string(),
        server_variant: true,
        asm: vec!["<mysteryToken>".to_string()],
        param_types: HashMap::new(),
    };

    let (bindings, warnings) =
        arkade_compiler::runtime::default_bindings_for_program_with_diagnostics(&program);

    assert_eq!(
        bindings.get("mysteryToken"),
        Some(&StackValue::Symbol("mysteryToken".to_string()))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("unknown placeholder 'mysteryToken'")),
        "expected warning for unknown placeholder, got: {warnings:?}"
    );
}
