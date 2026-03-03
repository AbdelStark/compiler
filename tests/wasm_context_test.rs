#![cfg(feature = "wasm")]

#[test]
fn wasm_context_json_propagated_execute_contract_json_uses_context_txid() {
    let contract_json = r#"{
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
}"#;

    let context_json = r#"{
  "tx_context": {
    "txid": "deadbeef00"
  }
}"#;

    let runtime_json =
        arkade_compiler::wasm::execute_contract_json(contract_json, "claim", context_json)
            .expect("execute_contract_json should return success");

    let payload: serde_json::Value =
        serde_json::from_str(&runtime_json).expect("WASM runtime output should be valid json");

    assert_eq!(payload["outcome"], "script_true");
}
