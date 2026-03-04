use std::process::Command;

#[test]
fn json_output_mode_parseable_cli_run_output_json_is_valid_envelope() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/htlc.json")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .arg("--output")
        .arg("json")
        .output()
        .expect("failed to run cli with json output mode");

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let payload: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be valid json");

    assert!(payload.get("status").is_some(), "missing status key");
    assert!(payload.get("result").is_some(), "missing result key");
    assert!(
        payload.get("schema_version").is_some(),
        "missing schema_version key"
    );
    assert_eq!(
        payload["status"], "true",
        "expected successful run status to be true"
    );
    assert!(
        payload["result"].is_object(),
        "expected result to be a structured object"
    );
    assert_eq!(payload["result"]["outcome"], "script_true");
    assert_eq!(payload["schema_version"], 1);
}
