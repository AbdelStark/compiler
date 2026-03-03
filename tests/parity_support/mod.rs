use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;

use arkade_compiler::runtime::env::{stack_value_to_bytes, ExecutionEnv};
use arkade_compiler::runtime::value::StackValue;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ParityVector {
    pub name: String,
    pub asm: Vec<String>,
    #[serde(default)]
    pub strict_placeholders: bool,
    #[serde(default)]
    pub bindings: BTreeMap<String, ValueSpec>,
    pub expected: ExpectedSpec,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ExpectedSpec {
    pub kind: String,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub top: Option<ValueSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValueSpec {
    Int {
        value: i64,
    },
    Bool {
        value: bool,
    },
    BytesHex {
        value: String,
    },
    BytesUtf8 {
        value: String,
    },
    Symbol {
        value: String,
    },
    PubkeyLabel {
        label: String,
    },
    TxGroupTxid {
        index: usize,
    },
    TxGroupGidx {
        index: usize,
    },
    SignatureLabel {
        label: String,
        #[serde(default)]
        message_key: Option<String>,
    },
}

pub fn load_vectors() -> Vec<ParityVector> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/parity_vectors");
    let mut entries = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed reading parity vector dir {}: {err}", root.display()))
        .map(|entry| entry.expect("failed reading parity vector entry").path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("json"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    entries.sort();

    let mut vectors = Vec::with_capacity(entries.len());
    for path in entries {
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed reading {}: {err}", path.display()));
        let vector: ParityVector = serde_json::from_str(&raw)
            .unwrap_or_else(|err| panic!("failed parsing {}: {err}", path.display()));
        vectors.push(vector);
    }

    vectors
}

pub fn materialize_bindings(
    env: &ExecutionEnv,
    specs: &BTreeMap<String, ValueSpec>,
) -> HashMap<String, StackValue> {
    let mut out: HashMap<String, StackValue> = HashMap::new();

    for (key, spec) in specs {
        match spec {
            ValueSpec::SignatureLabel { .. } => {}
            _ => {
                out.insert(key.clone(), to_stack_value(env, &out, spec));
            }
        }
    }

    for (key, spec) in specs {
        if let ValueSpec::SignatureLabel { .. } = spec {
            out.insert(key.clone(), to_stack_value(env, &out, spec));
        }
    }

    out
}

pub fn to_stack_value(
    env: &ExecutionEnv,
    existing_bindings: &HashMap<String, StackValue>,
    spec: &ValueSpec,
) -> StackValue {
    match spec {
        ValueSpec::Int { value } => StackValue::Int(*value),
        ValueSpec::Bool { value } => StackValue::Bool(*value),
        ValueSpec::BytesHex { value } => {
            let bytes = hex::decode(value).unwrap_or_else(|err| {
                panic!("invalid bytes_hex value '{value}': {err}");
            });
            StackValue::Bytes(bytes)
        }
        ValueSpec::BytesUtf8 { value } => StackValue::Bytes(value.as_bytes().to_vec()),
        ValueSpec::Symbol { value } => StackValue::Symbol(value.clone()),
        ValueSpec::PubkeyLabel { label } => {
            let (_secret, pk) = ExecutionEnv::derive_keypair_for_label(label);
            StackValue::Bytes(pk.serialize().to_vec())
        }
        ValueSpec::TxGroupTxid { index } => {
            let group = env
                .tx_context
                .asset_groups
                .get(*index)
                .unwrap_or_else(|| panic!("invalid tx group txid index {index}"));
            StackValue::Bytes(group.txid.clone())
        }
        ValueSpec::TxGroupGidx { index } => {
            let group = env
                .tx_context
                .asset_groups
                .get(*index)
                .unwrap_or_else(|| panic!("invalid tx group gidx index {index}"));
            StackValue::Int(group.gidx as i64)
        }
        ValueSpec::SignatureLabel { label, message_key } => {
            let message = message_key
                .as_ref()
                .and_then(|key| existing_bindings.get(key))
                .map(stack_value_to_bytes)
                .unwrap_or_else(|| env.tx_context.tx_hash.clone());
            let sig = ExecutionEnv::sign_message_for_label(label, &message);
            StackValue::Bytes(sig)
        }
    }
}
