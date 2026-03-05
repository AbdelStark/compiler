//! WASM bindings for the Arkade Compiler
//!
//! This module provides WebAssembly bindings for the compiler,
//! allowing it to be used in web browsers.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Initialize panic hook for better error messages in the browser console
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "wasm")]
    console_error_panic_hook::set_once();
}

/// Compile Arkade Script source code to JSON
///
/// # Arguments
/// * `source` - The Arkade Script source code
///
/// # Returns
/// A JSON string containing the compiled contract, or an error message
#[wasm_bindgen]
pub fn compile(source: &str) -> Result<String, String> {
    match crate::compiler::compile(source) {
        Ok(contract_json) => serde_json::to_string_pretty(&contract_json)
            .map_err(|e| format!("Serialization error: {}", e)),
        Err(e) => Err(e),
    }
}

/// Get the compiler version
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Validate Arkade Script source code without generating output
///
/// # Arguments
/// * `source` - The Arkade Script source code
///
/// # Returns
/// `true` if the source is valid, otherwise returns an error message
#[wasm_bindgen]
pub fn validate(source: &str) -> Result<bool, String> {
    match crate::compile(source) {
        Ok(_) => Ok(true),
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StackWireInput {
    Int { value: i64 },
    Bool { value: bool },
    BytesHex { value: String },
    BytesUtf8 { value: String },
    Symbol { value: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StackWireOutput {
    Int { value: i64 },
    Bool { value: bool },
    BytesHex { value: String },
    Symbol { value: String },
}

#[derive(Debug, Serialize)]
struct RuntimeStepWire {
    step_id: usize,
    ip: usize,
    token: String,
    status: String,
    elapsed_nanos: u64,
    policy_steps: usize,
    stack_before: Vec<StackWireOutput>,
    stack_after: Vec<StackWireOutput>,
}

#[derive(Debug, Serialize)]
struct RuntimeExecutionWire {
    contract_name: String,
    function_name: String,
    server_variant: bool,
    outcome: String,
    error_code: Option<String>,
    error_message: Option<String>,
    final_main_stack: Vec<StackWireOutput>,
    final_alt_stack: Vec<StackWireOutput>,
    telemetry: Vec<RuntimeStepWire>,
    trace_version: String,
    trace_id: String,
    seed: Option<u64>,
    runtime_options: serde_json::Value,
    policy_counters: serde_json::Value,
    elapsed_nanos: u64,
}

impl RuntimeExecutionWire {
    fn from_run(
        program: &crate::runtime::LoadedProgram,
        run: &crate::runtime::vm::VmRunResult,
    ) -> Self {
        let (outcome, error_code, error_message) = match &run.outcome {
            crate::runtime::vm::VmOutcome::ScriptTrue => ("script_true".to_string(), None, None),
            crate::runtime::vm::VmOutcome::ScriptFalse => ("script_false".to_string(), None, None),
            crate::runtime::vm::VmOutcome::RuntimeError(err) => (
                "runtime_error".to_string(),
                Some(format!("{:?}", err.code)),
                Some(err.to_string()),
            ),
        };

        let telemetry = run
            .telemetry
            .iter()
            .map(|step| RuntimeStepWire {
                step_id: step.step_id,
                ip: step.ip,
                token: step.token.clone(),
                status: match step.status {
                    crate::runtime::telemetry::StepStatus::Continue => "continue".to_string(),
                    crate::runtime::telemetry::StepStatus::ScriptTrue => "script_true".to_string(),
                    crate::runtime::telemetry::StepStatus::ScriptFalse => {
                        "script_false".to_string()
                    }
                    crate::runtime::telemetry::StepStatus::RuntimeError => {
                        "runtime_error".to_string()
                    }
                },
                elapsed_nanos: step.elapsed_nanos,
                policy_steps: step.policy_steps,
                stack_before: step.stack_before.iter().map(to_wire).collect(),
                stack_after: step.stack_after.iter().map(to_wire).collect(),
            })
            .collect();

        Self {
            contract_name: program.contract_name.clone(),
            function_name: program.function_name.clone(),
            server_variant: program.server_variant,
            outcome,
            error_code,
            error_message,
            final_main_stack: run.final_main_stack.iter().map(to_wire).collect(),
            final_alt_stack: run.final_alt_stack.iter().map(to_wire).collect(),
            telemetry,
            trace_version: run.trace_version.clone(),
            trace_id: run.trace_id.clone(),
            seed: run.seed,
            runtime_options: serde_json::to_value(&run.runtime_options)
                .unwrap_or(serde_json::Value::Null),
            policy_counters: serde_json::to_value(&run.policy_counters)
                .unwrap_or(serde_json::Value::Null),
            elapsed_nanos: run.elapsed_nanos,
        }
    }
}

fn to_wire(value: &crate::runtime::value::StackValue) -> StackWireOutput {
    match value {
        crate::runtime::value::StackValue::Int(value) => StackWireOutput::Int { value: *value },
        crate::runtime::value::StackValue::Bool(value) => StackWireOutput::Bool { value: *value },
        crate::runtime::value::StackValue::Bytes(value) => StackWireOutput::BytesHex {
            value: hex::encode(value),
        },
        crate::runtime::value::StackValue::Symbol(value) => StackWireOutput::Symbol {
            value: value.clone(),
        },
    }
}

fn from_wire(value: StackWireInput) -> Result<crate::runtime::value::StackValue, String> {
    Ok(match value {
        StackWireInput::Int { value } => crate::runtime::value::StackValue::Int(value),
        StackWireInput::Bool { value } => crate::runtime::value::StackValue::Bool(value),
        StackWireInput::BytesHex { value } => {
            let bytes = hex::decode(&value)
                .map_err(|err| format!("invalid bytes_hex binding value '{value}': {err}"))?;
            crate::runtime::value::StackValue::Bytes(bytes)
        }
        StackWireInput::BytesUtf8 { value } => {
            crate::runtime::value::StackValue::Bytes(value.into_bytes())
        }
        StackWireInput::Symbol { value } => crate::runtime::value::StackValue::Symbol(value),
    })
}

fn decode_bindings_json(
    raw: &str,
) -> Result<HashMap<String, crate::runtime::value::StackValue>, String> {
    if raw.trim().is_empty() {
        return Ok(HashMap::new());
    }

    let decoded: HashMap<String, StackWireInput> = serde_json::from_str(raw)
        .map_err(|err| format!("invalid runtime bindings json payload: {err}"))?;

    decoded
        .into_iter()
        .map(|(k, v)| Ok((k, from_wire(v)?)))
        .collect::<Result<HashMap<_, _>, _>>()
}

fn decode_context_json(
    context_json: Option<String>,
    context_strict: bool,
) -> Result<crate::runtime::env::TxContext, String> {
    match context_json {
        Some(raw) if !raw.trim().is_empty() => {
            crate::runtime::env::TxContext::from_json(&raw, context_strict)
                .map_err(|err| err.to_string())
        }
        _ => Ok(crate::runtime::env::TxContext::default()),
    }
}

fn decode_mode(mode: Option<String>) -> Result<crate::runtime::env::ExecutionMode, String> {
    match mode
        .unwrap_or_else(|| "development".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "development" => Ok(crate::runtime::env::ExecutionMode::Development),
        "simulation" => Ok(crate::runtime::env::ExecutionMode::Simulation),
        "ci" => Ok(crate::runtime::env::ExecutionMode::Ci),
        "safety" => Ok(crate::runtime::env::ExecutionMode::Safety),
        raw => Err(format!(
            "invalid execution mode '{raw}', expected development|simulation|ci|safety"
        )),
    }
}

/// Execute one function path from a compiled contract JSON artifact.
///
/// `bindings_json` must be a JSON object with values shaped like:
/// `{ "type": "int|bool|bytes_hex|bytes_utf8|symbol", "value": ... }`.
#[wasm_bindgen]
pub fn execute_contract_json(
    contract_json: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
) -> Result<String, String> {
    execute_contract_json_advanced(
        contract_json,
        function_name,
        server_variant,
        bindings_json,
        strict_placeholders,
        false,
        false,
        None,
        false,
        None,
    )
}

/// Execute one function path from a compiled contract JSON artifact with advanced runtime options.
#[wasm_bindgen]
pub fn execute_contract_json_advanced(
    contract_json: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
    strict_types: bool,
    strict_bindings: bool,
    context_json: Option<String>,
    context_strict: bool,
    mode: Option<String>,
) -> Result<String, String> {
    let contract: crate::models::ContractJson = serde_json::from_str(contract_json)
        .map_err(|err| format!("invalid contract artifact json: {err}"))?;
    let program =
        crate::runtime::load_program_from_contract(&contract, function_name, server_variant)
            .map_err(|err| err.to_string())?;

    let tx_context = decode_context_json(context_json, context_strict)?;
    let execution_mode = decode_mode(mode)?;

    let mut env = crate::runtime::env::ExecutionEnv::default().with_mode(execution_mode);
    env.strict_placeholders = strict_placeholders;
    env.strict_types = strict_types;
    env.strict_bindings = strict_bindings;
    env.tx_context = tx_context;
    env.bindings =
        crate::runtime::default_bindings_for_program_with_context(&program, &env.tx_context);
    env.bindings.extend(decode_bindings_json(bindings_json)?);

    let run = crate::runtime::execute_program(&program, &env);
    let output = RuntimeExecutionWire::from_run(&program, &run);
    serde_json::to_string_pretty(&output).map_err(|err| format!("Serialization error: {err}"))
}

/// Compile Ark source and execute one function path.
#[wasm_bindgen]
pub fn execute_source(
    source: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
) -> Result<String, String> {
    execute_source_advanced(
        source,
        function_name,
        server_variant,
        bindings_json,
        strict_placeholders,
        false,
        false,
        None,
        false,
        None,
    )
}

/// Compile Ark source and execute one function path with advanced runtime options.
#[wasm_bindgen]
pub fn execute_source_advanced(
    source: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
    strict_types: bool,
    strict_bindings: bool,
    context_json: Option<String>,
    context_strict: bool,
    mode: Option<String>,
) -> Result<String, String> {
    let contract = crate::compiler::compile(source)?;
    let contract_json =
        serde_json::to_string(&contract).map_err(|err| format!("Serialization error: {err}"))?;
    execute_contract_json_advanced(
        &contract_json,
        function_name,
        server_variant,
        bindings_json,
        strict_placeholders,
        strict_types,
        strict_bindings,
        context_json,
        context_strict,
        mode,
    )
}
