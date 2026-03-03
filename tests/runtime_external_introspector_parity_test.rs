mod parity_support;

use std::io::Write;
use std::process::Command;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use parity_support::{load_vectors_from_rel_dir, materialize_bindings};

#[derive(Debug, Serialize)]
struct ExternalParityInput {
    name: String,
    asm: Vec<String>,
    strict_placeholders: bool,
    bindings: std::collections::HashMap<String, StackWireValue>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StackWireValue {
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
fn introspector_runtime_parity_bridge_kind_only() {
    let enabled = std::env::var("ARKADE_ENABLE_INTROSPECTOR_PARITY")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !enabled {
        return;
    }

    let cmd = std::env::var("ARKADE_INTROSPECTOR_PARITY_CMD")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "cd tools/introspector_parity_adapter && go run . --".to_string());

    let vectors = load_vectors_from_rel_dir("tests/parity_vectors_introspector");
    assert!(!vectors.is_empty(), "introspector parity vectors are empty");

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
            .expect("failed executing introspector parity command");

        if !output.status.success() {
            mismatches.push(format!(
                "{} introspector adapter command failed: {}",
                vector.name,
                String::from_utf8_lossy(&output.stderr)
            ));
            continue;
        }

        let parsed: ExternalParityResult = match serde_json::from_slice(&output.stdout) {
            Ok(v) => v,
            Err(err) => {
                mismatches.push(format!(
                    "{} invalid introspector adapter json output: {} (stdout={})",
                    vector.name,
                    err,
                    String::from_utf8_lossy(&output.stdout)
                ));
                continue;
            }
        };

        if parsed.kind != local_kind {
            mismatches.push(format!(
                "{} kind mismatch: local={}, introspector={}",
                vector.name, local_kind, parsed.kind
            ));
        }
    }

    if !mismatches.is_empty() {
        panic!(
            "introspector parity mismatches (kind-only):\n{}",
            mismatches.join("\n")
        );
    }
}

fn shell_escape_single(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\"'\"'"))
}
