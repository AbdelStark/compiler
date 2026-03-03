#[cfg(not(target_arch = "wasm32"))]
pub mod debugger;
pub mod dispatcher;
pub mod env;
pub mod error;
pub mod stack;
pub mod telemetry;
pub mod value;
pub mod vm;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::models::{AbiFunction, ContractJson};
use crate::runtime::env::{stack_value_to_bytes, ExecutionEnv, TxContext};
use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::value::StackValue;
use crate::runtime::vm::{VMState, VmRunResult};

#[derive(Debug, Clone)]
pub struct LoadedProgram {
    pub contract_name: String,
    pub function_name: String,
    pub server_variant: bool,
    pub asm: Vec<String>,
    pub param_types: HashMap<String, String>,
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
    let mut param_types: HashMap<String, String> = HashMap::new();
    for input in &contract.parameters {
        param_types.insert(input.name.clone(), input.param_type.clone());
    }
    for input in &function.function_inputs {
        param_types.insert(input.name.clone(), input.param_type.clone());
    }

    Ok(LoadedProgram {
        contract_name: contract.name.clone(),
        function_name: function.name.clone(),
        server_variant,
        asm: function.asm.clone(),
        param_types,
    })
}

pub fn execute_program(program: &LoadedProgram, env: &ExecutionEnv) -> VmRunResult {
    let mut vm = VMState::new(program.asm.clone());
    vm.run(env)
}

pub fn default_bindings_for_program(program: &LoadedProgram) -> HashMap<String, StackValue> {
    let mut bindings: HashMap<String, StackValue> = HashMap::new();
    let tx_context = TxContext::default();

    let mut placeholders: HashSet<String> = HashSet::new();
    for token in &program.asm {
        if let Some(name) = parse_placeholder(token) {
            placeholders.insert(name.to_string());
        }
    }

    for name in &placeholders {
        if let Some(param_type) = program.param_types.get(name) {
            if apply_typed_default_binding(name, param_type, &mut bindings) {
                continue;
            }
        }

        if name == "preimage" {
            bindings.insert(name.clone(), StackValue::Bytes(b"hello".to_vec()));
            continue;
        }

        if name == "hash" {
            let digest = Sha256::digest(b"hello");
            bindings.insert(name.clone(), StackValue::Bytes(digest.to_vec()));
            continue;
        }

        if name.ends_with("_txid") {
            let idx = hash_to_index(name, tx_context.asset_groups.len());
            bindings.insert(
                name.clone(),
                StackValue::Bytes(tx_context.asset_groups[idx].txid.clone()),
            );
            continue;
        }

        if name.ends_with("_gidx") {
            let idx = hash_to_index(name, tx_context.asset_groups.len());
            bindings.insert(
                name.clone(),
                StackValue::Int(tx_context.asset_groups[idx].gidx as i64),
            );
            continue;
        }

        if is_probable_numeric(name) {
            bindings.insert(name.clone(), StackValue::Int(0));
            continue;
        }

        bindings.insert(name.clone(), StackValue::Symbol(name.clone()));
    }

    if !bindings.contains_key("refundTime") {
        bindings.insert("refundTime".to_string(), StackValue::Int(0));
    }

    let mut sig_map: HashMap<String, (String, Option<String>)> = HashMap::new();
    sig_map.extend(scan_checksig_patterns(&program.asm));
    sig_map.extend(scan_checkmultisig_patterns(&program.asm));

    for (sig, (pk, msg)) in &sig_map {
        ensure_pubkey_binding(pk, &mut bindings);

        let message = msg
            .as_ref()
            .and_then(|m| bindings.get(m).map(stack_value_to_bytes))
            .unwrap_or_else(|| tx_context.tx_hash.clone());

        let signature = ExecutionEnv::sign_message_for_label(pk, &message);
        bindings.insert(sig.clone(), StackValue::Bytes(signature));
    }

    for name in &placeholders {
        if is_probable_pubkey(name) {
            ensure_pubkey_binding(name, &mut bindings);
        }
    }

    if let Some(preimage) = bindings.get("preimage") {
        let digest = Sha256::digest(stack_value_to_bytes(preimage));
        bindings.insert("hash".to_string(), StackValue::Bytes(digest.to_vec()));
    }

    for value in bindings.values_mut() {
        if matches!(value, StackValue::Symbol(_)) {
            *value = StackValue::Int(0);
        }
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

fn parse_placeholder(token: &str) -> Option<&str> {
    if token.starts_with('<') && token.ends_with('>') {
        Some(&token[1..token.len() - 1])
    } else {
        None
    }
}

fn hash_to_index(label: &str, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let digest = Sha256::digest(label.as_bytes());
    digest[0] as usize % len
}

fn is_probable_numeric(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("time")
        || lower.contains("value")
        || lower.contains("amount")
        || lower.contains("count")
        || lower.contains("index")
        || lower.contains("group")
        || lower == "i"
        || lower == "j"
        || lower == "k"
}

fn apply_typed_default_binding(
    name: &str,
    param_type: &str,
    bindings: &mut HashMap<String, StackValue>,
) -> bool {
    match param_type {
        "pubkey" => {
            ensure_pubkey_binding(name, bindings);
            true
        }
        "signature" => {
            bindings
                .entry(name.to_string())
                .or_insert_with(|| StackValue::Bytes(vec![0u8; 64]));
            true
        }
        "bytes32" => {
            bindings
                .entry(name.to_string())
                .or_insert_with(|| StackValue::Bytes(vec![0u8; 32]));
            true
        }
        "bytes" => {
            bindings
                .entry(name.to_string())
                .or_insert_with(|| StackValue::Bytes(Vec::new()));
            true
        }
        "int" | "value" => {
            bindings
                .entry(name.to_string())
                .or_insert_with(|| StackValue::Int(0));
            true
        }
        "bool" => {
            bindings
                .entry(name.to_string())
                .or_insert_with(|| StackValue::Bool(true));
            true
        }
        _ => false,
    }
}

fn is_probable_pubkey(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    name == "SERVER_KEY"
        || lower.contains("pubkey")
        || lower.ends_with("key")
        || lower.ends_with("pk")
        || lower == "sender"
        || lower == "receiver"
        || lower == "owner"
        || lower == "server"
}

fn ensure_pubkey_binding(label: &str, bindings: &mut HashMap<String, StackValue>) {
    if matches!(bindings.get(label), Some(StackValue::Bytes(v)) if v.len() == 33 || v.len() == 65) {
        return;
    }

    let (_sk, pk) = ExecutionEnv::derive_keypair_for_label(label);
    bindings.insert(
        label.to_string(),
        StackValue::Bytes(pk.serialize().to_vec()),
    );
}

fn scan_checksig_patterns(script: &[String]) -> HashMap<String, (String, Option<String>)> {
    let mut out: HashMap<String, (String, Option<String>)> = HashMap::new();

    for i in 0..script.len() {
        let opcode = script[i].as_str();
        match opcode {
            "OP_CHECKSIG" | "OP_CHECKSIGVERIFY" => {
                if i >= 2 {
                    if let (Some(pk), Some(sig)) = (
                        parse_placeholder(&script[i - 2]),
                        parse_placeholder(&script[i - 1]),
                    ) {
                        out.insert(sig.to_string(), (pk.to_string(), None));
                    }
                }
            }
            "OP_CHECKSIGFROMSTACK" | "OP_CHECKSIGFROMSTACKVERIFY" => {
                if i >= 3 {
                    if let (Some(msg), Some(pk), Some(sig)) = (
                        parse_placeholder(&script[i - 3]),
                        parse_placeholder(&script[i - 2]),
                        parse_placeholder(&script[i - 1]),
                    ) {
                        out.insert(sig.to_string(), (pk.to_string(), Some(msg.to_string())));
                    }
                }
            }
            "OP_CHECKSIGADD" => {
                if i >= 3 {
                    if let (Some(sig), Some(pk)) = (
                        parse_placeholder(&script[i - 3]),
                        parse_placeholder(&script[i - 1]),
                    ) {
                        out.insert(sig.to_string(), (pk.to_string(), None));
                    }
                }
            }
            _ => {}
        }
    }

    out
}

fn scan_checkmultisig_patterns(script: &[String]) -> HashMap<String, (String, Option<String>)> {
    let mut out: HashMap<String, (String, Option<String>)> = HashMap::new();

    for i in 0..script.len() {
        if script[i] != "OP_CHECKMULTISIG" || i < 4 {
            continue;
        }

        let mut sig_cursor = i - 1;
        let mut signatures_rev: Vec<String> = Vec::new();
        while sig_cursor > 0 {
            if parse_push_number(&script[sig_cursor]).is_some() {
                break;
            }
            if let Some(sig) = parse_placeholder(&script[sig_cursor]) {
                signatures_rev.push(sig.to_string());
            }
            sig_cursor -= 1;
        }

        let sig_count = parse_push_number(&script[sig_cursor]).unwrap_or(0);
        let mut signatures = signatures_rev;
        signatures.reverse();
        if sig_count == 0 || signatures.len() < sig_count {
            continue;
        }
        signatures.truncate(sig_count);

        if sig_cursor == 0 {
            continue;
        }
        let mut key_cursor = sig_cursor - 1;
        let mut pubkeys_rev: Vec<String> = Vec::new();
        while key_cursor > 0 {
            if parse_push_number(&script[key_cursor]).is_some() {
                break;
            }
            if let Some(pk) = parse_placeholder(&script[key_cursor]) {
                pubkeys_rev.push(pk.to_string());
            }
            key_cursor -= 1;
        }

        let key_count = parse_push_number(&script[key_cursor]).unwrap_or(0);
        let mut pubkeys = pubkeys_rev;
        pubkeys.reverse();
        if key_count == 0 || pubkeys.len() < key_count {
            continue;
        }
        pubkeys.truncate(key_count);

        for (sig, pk) in signatures.into_iter().zip(pubkeys.into_iter()) {
            out.insert(sig, (pk, None));
        }
    }

    out
}

fn parse_push_number(token: &str) -> Option<usize> {
    if token == "OP_0" {
        return Some(0);
    }
    if let Some(rest) = token.strip_prefix("OP_") {
        if let Ok(v) = rest.parse::<usize>() {
            return Some(v);
        }
    }
    token.parse::<usize>().ok()
}
