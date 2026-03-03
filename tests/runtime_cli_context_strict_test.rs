use std::fs;
use std::process::Command;

#[test]
fn context_strict_rejects_unknown_fields_cli_run_rejects_rogue_field() {
    let temp = tempfile::tempdir().expect("tempdir should be created");
    let fixture_path = temp.path().join("fixture.json");

    fs::write(
        &fixture_path,
        r#"{
  "tx_context": {
    "txid": "0xabcd1234",
    "rogue_field": 42
  }
}"#,
    )
    .expect("context fixture should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/htlc.json")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .arg("--context-file")
        .arg(&fixture_path)
        .arg("--context-strict")
        .output()
        .expect("failed to run cli with strict context");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rogue_field"),
        "stderr should mention rogue_field, got: {stderr}"
    );
}
