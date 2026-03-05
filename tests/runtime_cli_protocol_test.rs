use std::fs;
use std::process::Command;

use arkade_compiler::runtime::env::TxContext;
use tempfile::tempdir;

#[test]
fn cli_run_json_output_includes_trace_metadata() {
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
        .expect("failed running arkadec");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let payload: serde_json::Value =
        serde_json::from_str(&stdout).expect("json output should be valid JSON payload");

    let result = payload
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .expect("results[0] should exist");

    assert_eq!(
        result.get("trace_version").and_then(|v| v.as_str()),
        Some("v1")
    );
    assert!(
        result
            .get("trace_id")
            .and_then(|v| v.as_str())
            .is_some_and(|v| !v.is_empty()),
        "trace_id should be present"
    );
}

#[test]
fn cli_run_lists_functions() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/htlc.json")
        .arg("--list-functions")
        .output()
        .expect("failed running arkadec");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("claim"), "stdout: {stdout}");
}

#[test]
fn cli_run_can_dump_default_context() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/htlc.json")
        .arg("--dump-default-context")
        .output()
        .expect("failed running arkadec");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"txid\""), "stdout: {stdout}");
}

#[test]
fn cli_run_context_strict_rejects_unknown_fields() {
    let dir = tempdir().expect("temp dir should be created");
    let context_path = dir.path().join("context.json");

    let mut value = serde_json::to_value(TxContext::default()).expect("default context serializes");
    value
        .as_object_mut()
        .expect("context value should be object")
        .insert("unexpectedField".to_string(), serde_json::json!(123));
    fs::write(
        &context_path,
        serde_json::to_string_pretty(&value).expect("context value should encode"),
    )
    .expect("context file should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/htlc.json")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .arg("--context-file")
        .arg(&context_path)
        .arg("--context-strict")
        .output()
        .expect("failed running arkadec");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown tx context field"),
        "stderr should mention strict context failure, got: {stderr}"
    );
}
