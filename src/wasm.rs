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
    ip: usize,
    token: String,
    status: String,
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

fn execute_contract_json_impl(
    contract_json: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
    context_json: &str,
) -> Result<String, String> {
    let contract: crate::models::ContractJson = serde_json::from_str(contract_json)
        .map_err(|err| format!("invalid contract artifact json: {err}"))?;
    let program =
        crate::runtime::load_program_from_contract(&contract, function_name, server_variant)
            .map_err(|err| err.to_string())?;
    let tx_context = crate::runtime::context_fixture::parse_context_json(context_json, false)
        .map_err(|err| format!("invalid runtime context json payload: {err}"))?;

    let mut env = crate::runtime::env::ExecutionEnv {
        strict_placeholders,
        bindings: crate::runtime::default_bindings_for_program(&program),
        tx_context,
    };
    env.bindings.extend(decode_bindings_json(bindings_json)?);

    let run = crate::runtime::execute_program(&program, &env);
    let output = RuntimeExecutionWire::from_run(&program, &run);
    serde_json::to_string_pretty(&output).map_err(|err| format!("Serialization error: {err}"))
}

/// Execute one function path from a compiled contract JSON artifact.
///
/// This convenience form uses default options:
/// - `server_variant`: `false`
/// - `bindings_json`: `""`
/// - `strict_placeholders`: `false`
#[wasm_bindgen]
pub fn execute_contract_json(
    contract_json: &str,
    function_name: &str,
    context_json: &str,
) -> Result<String, String> {
    execute_contract_json_impl(contract_json, function_name, false, "", false, context_json)
}

/// Execute one function path from a compiled contract JSON artifact.
///
/// `bindings_json` must be a JSON object with values shaped like:
/// `{ "type": "int|bool|bytes_hex|bytes_utf8|symbol", "value": ... }`.
#[wasm_bindgen(js_name = execute_contract_json_with_options)]
pub fn execute_contract_json_with_options(
    contract_json: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
    context_json: &str,
) -> Result<String, String> {
    execute_contract_json_impl(
        contract_json,
        function_name,
        server_variant,
        bindings_json,
        strict_placeholders,
        context_json,
    )
}

fn execute_source_impl(
    source: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
    context_json: &str,
) -> Result<String, String> {
    let contract = crate::compiler::compile(source)?;
    let contract_json =
        serde_json::to_string(&contract).map_err(|err| format!("Serialization error: {err}"))?;
    execute_contract_json_impl(
        &contract_json,
        function_name,
        server_variant,
        bindings_json,
        strict_placeholders,
        context_json,
    )
}

/// Compile Ark source and execute one function path.
///
/// This convenience form uses default options:
/// - `server_variant`: `false`
/// - `bindings_json`: `""`
/// - `strict_placeholders`: `false`
#[wasm_bindgen]
pub fn execute_source(
    source: &str,
    function_name: &str,
    context_json: &str,
) -> Result<String, String> {
    execute_source_impl(source, function_name, false, "", false, context_json)
}

/// Compile Ark source and execute one function path with explicit options.
#[wasm_bindgen(js_name = execute_source_with_options)]
pub fn execute_source_with_options(
    source: &str,
    function_name: &str,
    server_variant: bool,
    bindings_json: &str,
    strict_placeholders: bool,
    context_json: &str,
) -> Result<String, String> {
    execute_source_impl(
        source,
        function_name,
        server_variant,
        bindings_json,
        strict_placeholders,
        context_json,
    )
}
