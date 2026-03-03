use std::fs;
use std::process::Command;

#[test]
fn run_with_context_file_cli_run_uses_fixture_tx_context_from_file() {
    let temp = tempfile::tempdir().expect("tempdir should be created");
    let contract_path = temp.path().join("contract.json");
    let fixture_path = temp.path().join("fixture.json");

    fs::write(
        &contract_path,
        r#"{
  "contractName": "txhash_fixture_check",
  "constructorInputs": [],
  "functions": [
    {
      "name": "claim",
      "functionInputs": [],
      "serverVariant": false,
      "require": [],
      "asm": ["OP_TXHASH", "abcd1234", "OP_EQUAL"]
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

    let with_context = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg(&contract_path)
        .arg("--function")
        .arg("claim")
        .arg("--context-file")
        .arg(&fixture_path)
        .arg("--output")
        .arg("json")
        .output()
        .expect("failed to run cli with context fixture");

    assert_eq!(with_context.status.code(), Some(0));

    let with_context_stdout = String::from_utf8_lossy(&with_context.stdout);
    let with_context_json: serde_json::Value =
        serde_json::from_str(&with_context_stdout).expect("stdout should be valid json");
    assert_eq!(
        with_context_json["result"]["tx_context"]["tx_hash"],
        "abcd1234"
    );
    assert_eq!(with_context_json["result"]["outcome"], "script_true");

    let without_context = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg(&contract_path)
        .arg("--function")
        .arg("claim")
        .arg("--output")
        .arg("json")
        .output()
        .expect("failed to run cli without context fixture");

    assert_eq!(without_context.status.code(), Some(1));
    let without_context_stdout = String::from_utf8_lossy(&without_context.stdout);
    let without_context_json: serde_json::Value =
        serde_json::from_str(&without_context_stdout).expect("stdout should be valid json");
    assert_eq!(without_context_json["result"]["outcome"], "script_false");
}

#[test]
fn run_without_context_falls_back_cli_run_without_flags_uses_synthetic_fallback() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/htlc.json")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .output()
        .expect("failed to run cli without context flags");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("RESULT: true"), "stdout was: {stdout}");
}
