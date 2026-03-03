use std::collections::HashMap;
use std::fs;

use anyhow::{Context, Result};
use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::telemetry::StepStatus;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome, VmRunResult};
use serde::{Deserialize, Serialize};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(2);
    }
}

fn run() -> Result<()> {
    let input_path = std::env::args()
        .nth(1)
        .context("usage: arkade_parity_adapter <input.json>")?;

    let raw = fs::read_to_string(&input_path)
        .with_context(|| format!("failed reading parity input '{input_path}'"))?;
    let input: ExternalParityInput = serde_json::from_str(&raw)
        .with_context(|| format!("failed parsing parity input '{input_path}'"))?;

    let mut env = ExecutionEnv::default();
    env.strict_placeholders = input.strict_placeholders;
    env.bindings = decode_bindings(input.bindings)?;

    let mut vm = VMState::new(input.asm);
    let run = vm.run(&env);
    let output = ExternalParityResult::from_run(&run);

    println!(
        "{}",
        serde_json::to_string(&output).context("failed serializing parity output")?
    );
    Ok(())
}

fn decode_bindings(
    bindings: HashMap<String, StackWireValue>,
) -> Result<HashMap<String, StackValue>> {
    bindings
        .into_iter()
        .map(|(k, v)| Ok((k, from_wire(v)?)))
        .collect::<Result<HashMap<_, _>>>()
}

fn from_wire(value: StackWireValue) -> Result<StackValue> {
    Ok(match value {
        StackWireValue::Int { value } => StackValue::Int(value),
        StackWireValue::Bool { value } => StackValue::Bool(value),
        StackWireValue::BytesHex { value } => {
            let bytes = hex::decode(&value)
                .with_context(|| format!("invalid bytes_hex value '{value}'"))?;
            StackValue::Bytes(bytes)
        }
        StackWireValue::Symbol { value } => StackValue::Symbol(value),
    })
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

#[derive(Debug, Deserialize)]
struct ExternalParityInput {
    #[allow(dead_code)]
    name: String,
    asm: Vec<String>,
    strict_placeholders: bool,
    bindings: HashMap<String, StackWireValue>,
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
    final_main_stack: Vec<StackWireValue>,
    final_alt_stack: Vec<StackWireValue>,
    telemetry: Vec<StepWireTelemetry>,
}

impl ExternalParityResult {
    fn from_run(run: &VmRunResult) -> Self {
        let (kind, error_code) = match &run.outcome {
            VmOutcome::ScriptTrue => ("script_true".to_string(), None),
            VmOutcome::ScriptFalse => ("script_false".to_string(), None),
            VmOutcome::RuntimeError(err) => {
                ("runtime_error".to_string(), Some(format!("{:?}", err.code)))
            }
        };

        Self {
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
}
