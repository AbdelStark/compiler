mod parity_support;

use std::io::Write;
use std::process::Command;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use parity_support::{load_vectors, materialize_bindings};

#[derive(Debug, Serialize)]
struct ExternalParityInput {
    name: String,
    asm: Vec<String>,
    strict_placeholders: bool,
    bindings: std::collections::HashMap<String, BindingWireValue>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BindingWireValue {
    Int { value: i64 },
    Bool { value: bool },
    BytesHex { value: String },
    Symbol { value: String },
}

#[derive(Debug, Deserialize)]
struct ExternalParityResult {
    kind: String,
    #[allow(dead_code)]
    error_code: Option<String>,
}

#[test]
fn external_runtime_parity_bridge() {
    let cmd = match std::env::var("ARKADE_PARITY_EXTERNAL_CMD") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return,
    };

    let vectors = load_vectors();
    let mut mismatches: Vec<String> = Vec::new();

    for vector in vectors {
        let mut env = ExecutionEnv::default();
        env.strict_placeholders = vector.strict_placeholders;
        env.bindings = materialize_bindings(&env, &vector.bindings);

        let mut vm = VMState::new(vector.asm.clone());
        let local = vm.run(&env);
        let local_kind = match local.outcome {
            VmOutcome::ScriptTrue => "script_true",
            VmOutcome::ScriptFalse => "script_false",
            VmOutcome::RuntimeError(_) => "runtime_error",
        };

        let mut bindings = std::collections::HashMap::new();
        for (key, value) in &env.bindings {
            let wire = match value {
                arkade_compiler::runtime::value::StackValue::Int(v) => {
                    BindingWireValue::Int { value: *v }
                }
                arkade_compiler::runtime::value::StackValue::Bool(v) => {
                    BindingWireValue::Bool { value: *v }
                }
                arkade_compiler::runtime::value::StackValue::Bytes(v) => {
                    BindingWireValue::BytesHex {
                        value: hex::encode(v),
                    }
                }
                arkade_compiler::runtime::value::StackValue::Symbol(v) => {
                    BindingWireValue::Symbol { value: v.clone() }
                }
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

        let shell_cmd = format!("{} {}", cmd, temp.path().display());
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

        if parsed.kind != local_kind {
            mismatches.push(format!(
                "{} kind mismatch: local={}, external={}",
                vector.name, local_kind, parsed.kind
            ));
        }
    }

    if !mismatches.is_empty() {
        panic!("external parity mismatches:\n{}", mismatches.join("\n"));
    }
}
