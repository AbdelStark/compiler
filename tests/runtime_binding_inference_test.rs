use arkade_compiler::runtime::env::stack_value_to_bytes;
use arkade_compiler::runtime::value::StackValue;

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
