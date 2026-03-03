mod parity_support;

use std::io::Write;
use std::process::Command;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::telemetry::StepStatus;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome, VmRunResult};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use parity_support::{load_vectors, materialize_bindings};

#[derive(Debug, Serialize)]
struct ExternalParityInput {
    name: String,
    asm: Vec<String>,
    strict_placeholders: bool,
    bindings: std::collections::HashMap<String, StackWireValue>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StackWireValue {
    Int { value: i64 },
    Bool { value: bool },
    BytesHex { value: String },
    Symbol { value: String },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
struct StepWireTelemetry {
    ip: usize,
    token: String,
    status: String,
    stack_before: Vec<StackWireValue>,
    stack_after: Vec<StackWireValue>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
struct ExternalParityResult {
    kind: String,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    final_main_stack: Vec<StackWireValue>,
    #[serde(default)]
    final_alt_stack: Vec<StackWireValue>,
    #[serde(default)]
    telemetry: Vec<StepWireTelemetry>,
}

#[test]
fn external_runtime_parity_bridge() {
    let cmd = resolve_external_cmd().expect(
        "missing external parity adapter command; set ARKADE_PARITY_EXTERNAL_CMD or run via cargo test so CARGO_BIN_EXE_arkade_parity_adapter is present",
    );

    let vectors = load_vectors();
    let mut mismatches: Vec<String> = Vec::new();

    for vector in vectors {
        let mut env = ExecutionEnv::default();
        env.strict_placeholders = vector.strict_placeholders;
        env.bindings = materialize_bindings(&env, &vector.bindings);

        let mut vm = VMState::new(vector.asm.clone());
        let local = vm.run(&env);
        let expected = parity_result_from_run(&local);

        let mut bindings = std::collections::HashMap::new();
        for (key, value) in &env.bindings {
            let wire = match value {
                StackValue::Int(v) => StackWireValue::Int { value: *v },
                StackValue::Bool(v) => StackWireValue::Bool { value: *v },
                StackValue::Bytes(v) => StackWireValue::BytesHex {
                    value: hex::encode(v),
                },
                StackValue::Symbol(v) => StackWireValue::Symbol { value: v.clone() },
            };
            bindings.insert(key.clone(), wire);
        }

        let payload = ExternalParityInput {
            name: vector.name.clone(),
            asm: vector.asm,
            strict_placeholders: vector.strict_placeholders,
            bindings,
        };

        let mut temp = NamedTempFile::new().expect("failed creating temp file");
        let json = serde_json::to_string_pretty(&payload).expect("failed serializing payload");
        temp.write_all(json.as_bytes())
            .expect("failed writing payload file");

        let shell_cmd = format!(
            "{} {}",
            cmd,
            shell_escape_single(temp.path().to_string_lossy().as_ref())
        );
        let output = Command::new("sh")
            .arg("-lc")
            .arg(&shell_cmd)
            .output()
            .expect("failed executing external parity command");

        if !output.status.success() {
            mismatches.push(format!(
                "{} external command failed: {}",
                vector.name,
                String::from_utf8_lossy(&output.stderr)
            ));
            continue;
        }

        let parsed: ExternalParityResult = match serde_json::from_slice(&output.stdout) {
            Ok(v) => v,
            Err(err) => {
                mismatches.push(format!(
                    "{} invalid external json output: {} (stdout={})",
                    vector.name,
                    err,
                    String::from_utf8_lossy(&output.stdout)
                ));
                continue;
            }
        };

        if parsed.kind != expected.kind {
            mismatches.push(format!(
                "{} kind mismatch: local={}, external={}",
                vector.name, expected.kind, parsed.kind
            ));
            continue;
        }

        if parsed.error_code != expected.error_code {
            mismatches.push(format!(
                "{} error code mismatch: local={:?}, external={:?}",
                vector.name, expected.error_code, parsed.error_code
            ));
            continue;
        }

        if parsed.final_main_stack != expected.final_main_stack {
            mismatches.push(format!(
                "{} final_main_stack mismatch: local={:?}, external={:?}",
                vector.name, expected.final_main_stack, parsed.final_main_stack
            ));
            continue;
        }

        if parsed.final_alt_stack != expected.final_alt_stack {
            mismatches.push(format!(
                "{} final_alt_stack mismatch: local={:?}, external={:?}",
                vector.name, expected.final_alt_stack, parsed.final_alt_stack
            ));
            continue;
        }

        if parsed.telemetry != expected.telemetry {
            mismatches.push(format!(
                "{} telemetry mismatch: local_steps={}, external_steps={}",
                vector.name,
                expected.telemetry.len(),
                parsed.telemetry.len()
            ));
        }
    }

    if !mismatches.is_empty() {
        panic!("external parity mismatches:\n{}", mismatches.join("\n"));
    }
}

fn resolve_external_cmd() -> Option<String> {
    if let Ok(v) = std::env::var("ARKADE_PARITY_EXTERNAL_CMD") {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }

    std::env::var("CARGO_BIN_EXE_arkade_parity_adapter")
        .ok()
        .map(|path| shell_escape_single(&path))
        .or_else(|| Some("cargo run --quiet --bin arkade_parity_adapter --".to_string()))
}

fn shell_escape_single(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\"'\"'"))
}

fn parity_result_from_run(run: &VmRunResult) -> ExternalParityResult {
    let (kind, error_code) = match &run.outcome {
        VmOutcome::ScriptTrue => ("script_true".to_string(), None),
        VmOutcome::ScriptFalse => ("script_false".to_string(), None),
        VmOutcome::RuntimeError(err) => {
            ("runtime_error".to_string(), Some(format!("{:?}", err.code)))
        }
    };

    ExternalParityResult {
        kind,
        error_code,
        final_main_stack: run.final_main_stack.iter().map(to_wire).collect(),
        final_alt_stack: run.final_alt_stack.iter().map(to_wire).collect(),
        telemetry: run
            .telemetry
            .iter()
            .map(|step| StepWireTelemetry {
                ip: step.ip,
                token: step.token.clone(),
                status: status_to_wire(&step.status),
                stack_before: step.stack_before.iter().map(to_wire).collect(),
                stack_after: step.stack_after.iter().map(to_wire).collect(),
            })
            .collect(),
    }
}

fn to_wire(value: &StackValue) -> StackWireValue {
    match value {
        StackValue::Int(value) => StackWireValue::Int { value: *value },
        StackValue::Bool(value) => StackWireValue::Bool { value: *value },
        StackValue::Bytes(value) => StackWireValue::BytesHex {
            value: hex::encode(value),
        },
        StackValue::Symbol(value) => StackWireValue::Symbol {
            value: value.clone(),
        },
    }
}

fn status_to_wire(status: &StepStatus) -> String {
    match status {
        StepStatus::Continue => "continue".to_string(),
        StepStatus::ScriptTrue => "script_true".to_string(),
        StepStatus::ScriptFalse => "script_false".to_string(),
        StepStatus::RuntimeError => "runtime_error".to_string(),
    }
}
