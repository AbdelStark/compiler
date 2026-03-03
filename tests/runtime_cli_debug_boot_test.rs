use std::process::Command;

#[test]
fn cli_debug_headless_initializes_panes_and_controls() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("debug")
        .arg("examples/htlc.json")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .arg("--headless")
        .output()
        .expect("failed to run debug cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Debugger initialized"), "stdout: {stdout}");
    assert!(stdout.contains("Script"), "stdout: {stdout}");
    assert!(
        stdout.contains("Main Stack / Alt Stack"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Telemetry Logs"), "stdout: {stdout}");
    assert!(stdout.contains("Step Over"), "stdout: {stdout}");
    assert!(stdout.contains("Continue"), "stdout: {stdout}");
    assert!(stdout.contains("Reset"), "stdout: {stdout}");
    assert!(stdout.contains("Toggle Breakpoint"), "stdout: {stdout}");
}

#[test]
fn cli_debug_headless_accepts_breakpoint_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("debug")
        .arg("examples/htlc.json")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .arg("--headless")
        .arg("--breakpoint")
        .arg("2")
        .arg("--breakpoint")
        .arg("6")
        .output()
        .expect("failed to run debug cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Breakpoints: 2,6"), "stdout: {stdout}");
}

#[test]
fn cli_debug_headless_supports_ark_source_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("debug")
        .arg("examples/htlc.ark")
        .arg("--function")
        .arg("claim")
        .arg("--variant")
        .arg("false")
        .arg("--headless")
        .output()
        .expect("failed to run debug cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Debugger initialized"), "stdout: {stdout}");
    assert!(
        stdout.contains("Debug source: examples/htlc.ark"),
        "stdout: {stdout}"
    );
}

#[test]
fn cli_debug_list_samples_prints_examples() {
    let output = Command::new(env!("CARGO_BIN_EXE_arkadec"))
        .arg("debug")
        .arg("--list-samples")
        .output()
        .expect("failed to run debug cli");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Available sample contracts"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("examples/htlc.ark"), "stdout: {stdout}");
    assert!(stdout.contains("examples/htlc.json"), "stdout: {stdout}");
}
