pub mod debugger;
pub mod dispatcher;
pub mod env;
pub mod error;
pub mod stack;
pub mod telemetry;
pub mod value;
pub mod vm;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::models::{AbiFunction, ContractJson};
use crate::runtime::env::ExecutionEnv;
use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::value::StackValue;
use crate::runtime::vm::{VMState, VmRunResult};

#[derive(Debug, Clone)]
pub struct LoadedProgram {
    pub contract_name: String,
    pub function_name: String,
    pub server_variant: bool,
    pub asm: Vec<String>,
}

pub fn load_program_from_file(
    path: impl AsRef<Path>,
    function_name: &str,
    server_variant: bool,
) -> Result<LoadedProgram, RuntimeError> {
    let raw = fs::read_to_string(path.as_ref()).map_err(|err| {
        RuntimeError::new(
            RuntimeErrorCode::Io,
            format!(
                "failed reading artifact '{}': {err}",
                path.as_ref().display()
            ),
        )
    })?;
    let contract: ContractJson = serde_json::from_str(&raw).map_err(|err| {
        RuntimeError::new(
            RuntimeErrorCode::Json,
            format!(
                "failed parsing artifact '{}': {err}",
                path.as_ref().display()
            ),
        )
    })?;
    load_program_from_contract(&contract, function_name, server_variant)
}

pub fn load_program_from_contract(
    contract: &ContractJson,
    function_name: &str,
    server_variant: bool,
) -> Result<LoadedProgram, RuntimeError> {
    let function = select_function(contract, function_name, server_variant)?;
    Ok(LoadedProgram {
        contract_name: contract.name.clone(),
        function_name: function.name.clone(),
        server_variant,
        asm: function.asm.clone(),
    })
}

pub fn execute_program(program: &LoadedProgram, env: &ExecutionEnv) -> VmRunResult {
    let mut vm = VMState::new(program.asm.clone());
    vm.run(env)
}

pub fn default_bindings_for_program(program: &LoadedProgram) -> HashMap<String, StackValue> {
    let mut bindings: HashMap<String, StackValue> = HashMap::new();

    for token in &program.asm {
        if token.starts_with('<') && token.ends_with('>') {
            let key = token[1..token.len() - 1].to_string();
            bindings
                .entry(key.clone())
                .or_insert_with(|| StackValue::Symbol(key));
        }
    }

    match bindings.get("preimage") {
        Some(StackValue::Bytes(_)) => {}
        _ => {
            bindings.insert("preimage".to_string(), StackValue::Bytes(b"hello".to_vec()));
        }
    }

    let digest = if let Some(preimage) = bindings.get("preimage") {
        Sha256::digest(preimage.to_bytes()).to_vec()
    } else {
        Sha256::digest(b"hello").to_vec()
    };
    match bindings.get("hash") {
        Some(StackValue::Bytes(_)) => {}
        _ => {
            bindings.insert("hash".to_string(), StackValue::Bytes(digest));
        }
    }

    if !bindings.contains_key("refundTime") {
        bindings.insert("refundTime".to_string(), StackValue::Int(0));
    }
    if !bindings.contains_key("SERVER_KEY") {
        bindings.insert(
            "SERVER_KEY".to_string(),
            StackValue::Symbol("SERVER_KEY".to_string()),
        );
    }
    if !bindings.contains_key("serverSig") {
        bindings.insert(
            "serverSig".to_string(),
            StackValue::Symbol("serverSig".to_string()),
        );
    }

    bindings
}

fn select_function<'a>(
    contract: &'a ContractJson,
    function_name: &str,
    server_variant: bool,
) -> Result<&'a AbiFunction, RuntimeError> {
    contract
        .functions
        .iter()
        .find(|f| f.name == function_name && f.server_variant == server_variant)
        .ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::FunctionNotFound,
                format!(
                    "function '{}' with serverVariant={} not found",
                    function_name, server_variant
                ),
            )
        })
}
