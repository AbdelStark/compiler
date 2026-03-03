use std::process::Command;

#[test]
fn cli_run_succeeds_for_htlc_claim_exit_variant() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/htlc.json")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .output()
        .expect("failed to run cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("RESULT: true"), "stdout was: {stdout}");
}

#[test]
fn cli_run_succeeds_for_htlc_ark_source_claim_exit_variant() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/htlc.ark")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .output()
        .expect("failed to run cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("RESULT: true"), "stdout was: {stdout}");
}

#[test]
fn cli_run_missing_file_returns_exit_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/does_not_exist.json")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .output()
        .expect("failed to run cli");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("runtime_error"),
        "stderr did not contain runtime_error: {stderr}"
    );
}

#[test]
fn cli_run_trace_emits_opcode_step_logs() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("run")
        .arg("examples/htlc.json")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .arg("--trace")
        .output()
        .expect("failed to run cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains("OP_SHA256"),
        "expected trace output to include opcode logs, output was: {combined}"
    );
}
