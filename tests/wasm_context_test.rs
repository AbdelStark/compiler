fn wasm_contract_json() -> &'static str {
    r#"{
  "contractName": "wasm_txhash_check",
  "constructorInputs": [],
  "functions": [
    {
      "name": "claim",
      "functionInputs": [],
      "serverVariant": false,
      "require": [],
      "asm": ["OP_TXHASH", "deadbeef00", "OP_EQUAL"]
    }
  ]
}"#
}

fn wasm_context_json() -> &'static str {
    r#"{
  "tx_context": {
    "txid": "deadbeef00"
  }
}"#
}

fn assert_runtime_tx_hash(runtime_json: &str, expected: &str) {
    let payload: serde_json::Value =
        serde_json::from_str(runtime_json).expect("WASM runtime output should be valid json");
    assert_eq!(payload["tx_context"]["tx_hash"], expected);
    assert_eq!(payload["outcome"], "script_true");
}

#[test]
fn wasm_context_json_propagated_execute_contract_json_uses_context_txid() {
    let runtime_json = arkade_compiler::wasm::execute_contract_json(
        wasm_contract_json(),
        "claim",
        wasm_context_json(),
    )
    .expect("execute_contract_json should return success");

    assert_runtime_tx_hash(&runtime_json, "deadbeef00");
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn wasm_context_json_propagated_execute_contract_json_uses_context_txid_in_wasm_runtime() {
    let runtime_json = arkade_compiler::wasm::execute_contract_json(
        wasm_contract_json(),
        "claim",
        wasm_context_json(),
    )
    .expect("execute_contract_json should return success");

    assert_runtime_tx_hash(&runtime_json, "deadbeef00");
}

#[test]
fn wasm_context_strict_rejects_unknown_fields_with_options() {
    let contract_json = r#"{
  "contractName": "wasm_context_strict_check",
  "constructorInputs": [],
  "functions": [
    {
      "name": "claim",
      "functionInputs": [],
      "serverVariant": false,
      "require": [],
      "asm": ["OP_1"]
    }
  ]
}"#;

    let context_json = r#"{
  "tx_context": {
    "txid": "deadbeef00",
    "rogue_field": 42
  }
}"#;

    let err = arkade_compiler::wasm::execute_contract_json_with_options(
        contract_json,
        "claim",
        false,
        "",
        false,
        context_json,
        Some(true),
    )
    .expect_err("strict mode should reject unknown fields");

    assert!(
        err.contains("rogue_field"),
        "error should mention rogue_field, got: {err}"
    );
}
