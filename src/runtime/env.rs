use std::collections::{BTreeSet, HashMap};

#[cfg(not(target_arch = "wasm32"))]
use secp256k1::ecdsa::Signature as EcdsaSignature;
#[cfg(not(target_arch = "wasm32"))]
use secp256k1::schnorr::Signature as SchnorrSignature;
#[cfg(not(target_arch = "wasm32"))]
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::value::StackValue;

mod serde_hex {
    use serde::de::{self, SeqAccess, Visitor};
    use serde::{Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S>(value: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(value))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BytesVisitor;

        impl<'de> Visitor<'de> for BytesVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("hex string or byte array")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let trimmed = value.trim();
                let raw = trimmed.strip_prefix("0x").unwrap_or(trimmed);
                hex::decode(raw)
                    .map_err(|err| E::custom(format!("invalid hex bytes '{value}': {err}")))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut out = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    out.push(byte);
                }
                Ok(out)
            }
        }

        deserializer.deserialize_any(BytesVisitor)
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretKey([u8; 32]);

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey([u8; 33]);

#[cfg(target_arch = "wasm32")]
impl PublicKey {
    pub fn serialize(&self) -> [u8; 33] {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    #[serde(with = "serde_hex")]
    pub txid: Vec<u8>,
    pub gidx: u16,
    pub amount: i64,
    #[serde(with = "serde_hex")]
    pub data: Vec<u8>,
    #[serde(with = "serde_hex")]
    pub control: Vec<u8>,
    #[serde(rename = "metadataHash", with = "serde_hex")]
    pub metadata_hash: Vec<u8>,
    #[serde(rename = "assetId", with = "serde_hex")]
    pub asset_id: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub value: i64,
    #[serde(rename = "scriptPubKey", with = "serde_hex")]
    pub script_pubkey: Vec<u8>,
    pub sequence: i64,
    #[serde(with = "serde_hex")]
    pub outpoint: Vec<u8>,
    #[serde(with = "serde_hex")]
    pub issuance: Vec<u8>,
    #[serde(default)]
    pub assets: Vec<AssetEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: i64,
    #[serde(rename = "scriptPubKey", with = "serde_hex")]
    pub script_pubkey: Vec<u8>,
    #[serde(with = "serde_hex")]
    pub nonce: Vec<u8>,
    #[serde(default)]
    pub assets: Vec<AssetEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetGroup {
    #[serde(with = "serde_hex")]
    pub txid: Vec<u8>,
    pub gidx: u16,
    #[serde(rename = "sumInputs")]
    pub sum_inputs: i64,
    #[serde(rename = "sumOutputs")]
    pub sum_outputs: i64,
    #[serde(rename = "numInputs")]
    pub num_inputs: i64,
    #[serde(rename = "numOutputs")]
    pub num_outputs: i64,
    #[serde(with = "serde_hex")]
    pub control: Vec<u8>,
    #[serde(rename = "metadataHash", with = "serde_hex")]
    pub metadata_hash: Vec<u8>,
    #[serde(rename = "assetId", with = "serde_hex")]
    pub asset_id: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxContext {
    #[serde(rename = "txid", alias = "tx_hash", with = "serde_hex")]
    pub tx_hash: Vec<u8>,
    pub version: i64,
    pub locktime: i64,
    pub weight: i64,
    #[serde(rename = "currentInputIndex")]
    pub current_input_index: usize,
    #[serde(default)]
    pub inputs: Vec<TxInput>,
    #[serde(default)]
    pub outputs: Vec<TxOutput>,
    #[serde(rename = "assetGroups", default)]
    pub asset_groups: Vec<AssetGroup>,
}

impl TxContext {
    pub fn sample() -> Self {
        let mk32 = |tag: &str| Sha256::digest(tag.as_bytes()).to_vec();
        let mut groups: Vec<AssetGroup> = Vec::new();

        for idx in 0..4u16 {
            let txid = mk32(&format!("asset-group-txid-{idx}"));
            let metadata_hash = mk32(&format!("metadata-{idx}"));
            let control = format!("control-{idx}").into_bytes();
            let mut asset_id_seed = txid.clone();
            asset_id_seed.extend_from_slice(&idx.to_le_bytes());
            let asset_id = Sha256::digest(&asset_id_seed).to_vec();

            groups.push(AssetGroup {
                txid,
                gidx: idx,
                sum_inputs: 10_000 + idx as i64 * 100,
                sum_outputs: 9_500 + idx as i64 * 100,
                num_inputs: 2,
                num_outputs: 2,
                control,
                metadata_hash,
                asset_id,
            });
        }

        let input_assets = groups
            .iter()
            .enumerate()
            .map(|(i, g)| AssetEntry {
                txid: g.txid.clone(),
                gidx: g.gidx,
                amount: 5_000 + i as i64,
                data: format!("in-asset-{i}").into_bytes(),
                control: g.control.clone(),
                metadata_hash: g.metadata_hash.clone(),
                asset_id: g.asset_id.clone(),
            })
            .collect::<Vec<_>>();

        let output_assets = groups
            .iter()
            .enumerate()
            .map(|(i, g)| AssetEntry {
                txid: g.txid.clone(),
                gidx: g.gidx,
                amount: 4_750 + i as i64,
                data: format!("out-asset-{i}").into_bytes(),
                control: g.control.clone(),
                metadata_hash: g.metadata_hash.clone(),
                asset_id: g.asset_id.clone(),
            })
            .collect::<Vec<_>>();

        let inputs = (0..4)
            .map(|i| TxInput {
                value: 100_000 + i as i64,
                script_pubkey: format!("input-script-{i}").into_bytes(),
                sequence: 0xFFFF_FFF0 + i as i64,
                outpoint: mk32(&format!("outpoint-{i}")),
                issuance: format!("issuance-{i}").into_bytes(),
                assets: input_assets.clone(),
            })
            .collect::<Vec<_>>();

        let outputs = (0..4)
            .map(|i| TxOutput {
                value: 99_000 + i as i64,
                script_pubkey: format!("output-script-{i}").into_bytes(),
                nonce: mk32(&format!("nonce-{i}")),
                assets: output_assets.clone(),
            })
            .collect::<Vec<_>>();

        Self {
            tx_hash: mk32("arkade-runtime-default-tx-hash"),
            version: 2,
            locktime: 0,
            weight: 850,
            current_input_index: 0,
            inputs,
            outputs,
            asset_groups: groups,
        }
    }

    pub fn from_json(raw: &str, strict_unknown_fields: bool) -> Result<Self, RuntimeError> {
        if !strict_unknown_fields {
            return serde_json::from_str(raw).map_err(|err| {
                RuntimeError::new(
                    RuntimeErrorCode::Json,
                    format!("invalid tx context json: {err}"),
                )
            });
        }

        let mut unknown_paths = Vec::new();
        let mut deserializer = serde_json::Deserializer::from_str(raw);
        let context: TxContext = serde_ignored::deserialize(&mut deserializer, |path| {
            unknown_paths.push(path.to_string());
        })
        .map_err(|err| {
            RuntimeError::new(
                RuntimeErrorCode::Json,
                format!("invalid tx context json: {err}"),
            )
        })?;

        if unknown_paths.is_empty() {
            Ok(context)
        } else {
            Err(RuntimeError::new(
                RuntimeErrorCode::Json,
                format!("unknown tx context field(s): {}", unknown_paths.join(", ")),
            ))
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, RuntimeError> {
        serde_json::to_string_pretty(self).map_err(|err| {
            RuntimeError::new(
                RuntimeErrorCode::Json,
                format!("failed serializing tx context: {err}"),
            )
        })
    }

    pub fn input_at(&self, index: usize) -> Option<&TxInput> {
        self.inputs.get(index)
    }

    pub fn output_at(&self, index: usize) -> Option<&TxOutput> {
        self.outputs.get(index)
    }

    pub fn group_at(&self, index: usize) -> Option<&AssetGroup> {
        self.asset_groups.get(index)
    }

    pub fn find_group_index(&self, txid: &[u8], gidx: u16) -> i64 {
        self.asset_groups
            .iter()
            .position(|g| g.txid == txid && g.gidx == gidx)
            .map(|idx| idx as i64)
            .unwrap_or(-1)
    }
}

impl Default for TxContext {
    fn default() -> Self {
        Self::sample()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Development,
    Simulation,
    Ci,
    Safety,
}

impl ExecutionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Simulation => "simulation",
            Self::Ci => "ci",
            Self::Safety => "safety",
        }
    }
}

impl Default for ExecutionMode {
    fn default() -> Self {
        Self::Development
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePolicy {
    #[serde(default)]
    pub max_steps: Option<usize>,
    #[serde(default)]
    pub max_script_len: Option<usize>,
    #[serde(default)]
    pub max_main_stack_depth: Option<usize>,
    #[serde(default)]
    pub max_alt_stack_depth: Option<usize>,
    #[serde(default)]
    pub max_stack_growth_per_step: Option<usize>,
    #[serde(default)]
    pub max_opcode_budget: Option<usize>,
    #[serde(default)]
    pub allowed_opcodes: Option<BTreeSet<String>>,
}

impl RuntimePolicy {
    pub fn preset(mode: ExecutionMode) -> Self {
        match mode {
            ExecutionMode::Development => Self::default(),
            ExecutionMode::Simulation => Self {
                max_steps: Some(50_000),
                max_script_len: Some(20_000),
                max_main_stack_depth: Some(2_000),
                max_alt_stack_depth: Some(2_000),
                max_stack_growth_per_step: Some(128),
                max_opcode_budget: Some(50_000),
                allowed_opcodes: None,
            },
            ExecutionMode::Ci => Self {
                max_steps: Some(25_000),
                max_script_len: Some(10_000),
                max_main_stack_depth: Some(1_500),
                max_alt_stack_depth: Some(1_500),
                max_stack_growth_per_step: Some(64),
                max_opcode_budget: Some(25_000),
                allowed_opcodes: None,
            },
            ExecutionMode::Safety => Self {
                max_steps: Some(10_000),
                max_script_len: Some(5_000),
                max_main_stack_depth: Some(1_000),
                max_alt_stack_depth: Some(1_000),
                max_stack_growth_per_step: Some(32),
                max_opcode_budget: Some(10_000),
                allowed_opcodes: None,
            },
        }
    }

    pub fn with_overrides(mut self, other: &RuntimePolicy) -> Self {
        if other.max_steps.is_some() {
            self.max_steps = other.max_steps;
        }
        if other.max_script_len.is_some() {
            self.max_script_len = other.max_script_len;
        }
        if other.max_main_stack_depth.is_some() {
            self.max_main_stack_depth = other.max_main_stack_depth;
        }
        if other.max_alt_stack_depth.is_some() {
            self.max_alt_stack_depth = other.max_alt_stack_depth;
        }
        if other.max_stack_growth_per_step.is_some() {
            self.max_stack_growth_per_step = other.max_stack_growth_per_step;
        }
        if other.max_opcode_budget.is_some() {
            self.max_opcode_budget = other.max_opcode_budget;
        }
        if other.allowed_opcodes.is_some() {
            self.allowed_opcodes = other.allowed_opcodes.clone();
        }
        self
    }
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            max_steps: None,
            max_script_len: None,
            max_main_stack_depth: None,
            max_alt_stack_depth: None,
            max_stack_growth_per_step: None,
            max_opcode_budget: None,
            allowed_opcodes: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SignerContext {
    #[serde(default)]
    pub message_template_id: Option<String>,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

#[derive(Clone)]
pub struct ExecutionEnv {
    pub bindings: HashMap<String, StackValue>,
    pub strict_placeholders: bool,
    pub strict_types: bool,
    pub strict_bindings: bool,
    pub tx_context: TxContext,
    pub execution_mode: ExecutionMode,
    pub runtime_policy: RuntimePolicy,
    pub signer_context: SignerContext,
    pub seed: Option<u64>,
}

impl ExecutionEnv {
    pub fn new() -> Self {
        let execution_mode = ExecutionMode::default();
        Self {
            bindings: HashMap::new(),
            strict_placeholders: false,
            strict_types: false,
            strict_bindings: false,
            tx_context: TxContext::default(),
            execution_mode,
            runtime_policy: RuntimePolicy::preset(execution_mode),
            signer_context: SignerContext::default(),
            seed: None,
        }
    }

    pub fn resolve_placeholder(&self, key: &str) -> Result<StackValue, RuntimeError> {
        if let Some(v) = self.bindings.get(key) {
            return Ok(v.clone());
        }

        if self.strict_placeholders || self.strict_bindings {
            return Err(RuntimeError::new(
                RuntimeErrorCode::MissingBinding,
                format!("missing binding for placeholder '{key}'"),
            ));
        }

        Ok(StackValue::Symbol(key.to_string()))
    }

    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self.runtime_policy = RuntimePolicy::preset(mode).with_overrides(&self.runtime_policy);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn verify_signature(
        &self,
        pubkey: &StackValue,
        signature: &StackValue,
        message: Option<&StackValue>,
    ) -> bool {
        let secp = Secp256k1::verification_only();
        let pk_bytes = stack_value_to_bytes(pubkey);
        let sig_bytes = stack_value_to_bytes(signature);

        let parsed_pubkey = match PublicKey::from_slice(&pk_bytes) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let message_bytes = self.signature_message(message);
        let digest = Sha256::digest(&message_bytes);

        let msg = Message::from_digest(digest.into());
        if let Ok(ecdsa_sig) = EcdsaSignature::from_der(&sig_bytes) {
            if secp.verify_ecdsa(msg, &ecdsa_sig, &parsed_pubkey).is_ok() {
                return true;
            }
        }

        if sig_bytes.len() == 64 {
            let sig_arr: [u8; 64] = sig_bytes
                .as_slice()
                .try_into()
                .expect("checked schnorr signature length");
            let schnorr_sig = SchnorrSignature::from_byte_array(sig_arr);
            let (xonly, _) = parsed_pubkey.x_only_public_key();
            if secp.verify_schnorr(&schnorr_sig, &digest, &xonly).is_ok() {
                return true;
            }
        }

        false
    }

    #[cfg(target_arch = "wasm32")]
    pub fn verify_signature(
        &self,
        pubkey: &StackValue,
        signature: &StackValue,
        message: Option<&StackValue>,
    ) -> bool {
        let pk_bytes = stack_value_to_bytes(pubkey);
        let sig_bytes = stack_value_to_bytes(signature);
        if pk_bytes.len() != 33 || sig_bytes.len() != 32 {
            return false;
        }

        let message_bytes = self.signature_message(message);
        let expected = Self::sign_message_for_pubkey_bytes(&pk_bytes, &message_bytes);
        sig_bytes == expected
    }

    pub fn verify_multisig(&self, pubkeys: &[StackValue], signatures: &[StackValue]) -> bool {
        if signatures.len() > pubkeys.len() {
            return false;
        }

        signatures
            .iter()
            .zip(pubkeys.iter())
            .all(|(sig, pk)| self.verify_signature(pk, sig, None))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn derive_keypair_for_label(label: &str) -> (SecretKey, PublicKey) {
        let secp = Secp256k1::signing_only();
        let mut seed: [u8; 32] = Sha256::digest(label.as_bytes()).into();
        loop {
            if let Ok(secret) = SecretKey::from_byte_array(seed) {
                let public = PublicKey::from_secret_key(&secp, &secret);
                return (secret, public);
            }
            seed = Sha256::digest(seed).into();
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn derive_keypair_for_label(label: &str) -> (SecretKey, PublicKey) {
        let seed: [u8; 32] = Sha256::digest(label.as_bytes()).into();
        let mut compressed = [0u8; 33];
        compressed[0] = 0x02;
        compressed[1..].copy_from_slice(&seed);
        (SecretKey(seed), PublicKey(compressed))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sign_message_for_label(label: &str, message: &[u8]) -> Vec<u8> {
        Self::sign_message_for_label_with_template(label, message, None)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn sign_message_for_label(label: &str, message: &[u8]) -> Vec<u8> {
        Self::sign_message_for_label_with_template(label, message, None)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn sign_message_for_label_with_template(
        label: &str,
        message: &[u8],
        template_id: Option<&str>,
    ) -> Vec<u8> {
        let secp = Secp256k1::signing_only();
        let (secret, _) = Self::derive_keypair_for_label(label);
        let canonical = canonical_signature_message(message, template_id);
        let digest = Sha256::digest(canonical);
        let msg = Message::from_digest(digest.into());
        secp.sign_ecdsa(msg, &secret).serialize_der().to_vec()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn sign_message_for_label_with_template(
        label: &str,
        message: &[u8],
        template_id: Option<&str>,
    ) -> Vec<u8> {
        let (_secret, public) = Self::derive_keypair_for_label(label);
        let canonical = canonical_signature_message(message, template_id);
        Self::sign_message_for_pubkey_bytes(&public.serialize(), &canonical)
    }

    fn signature_message(&self, message: Option<&StackValue>) -> Vec<u8> {
        let message = message
            .map(stack_value_to_bytes)
            .unwrap_or_else(|| self.tx_context.tx_hash.clone());
        canonical_signature_message(&message, self.signer_context.message_template_id.as_deref())
    }

    #[cfg(target_arch = "wasm32")]
    fn sign_message_for_pubkey_bytes(pubkey_bytes: &[u8], message: &[u8]) -> Vec<u8> {
        let mut material = Vec::with_capacity(pubkey_bytes.len() + message.len());
        material.extend_from_slice(pubkey_bytes);
        material.extend_from_slice(message);
        Sha256::digest(material).to_vec()
    }
}

impl Default for ExecutionEnv {
    fn default() -> Self {
        Self::new()
    }
}

fn canonical_signature_message(message: &[u8], template_id: Option<&str>) -> Vec<u8> {
    match template_id {
        Some(template_id) if !template_id.is_empty() => {
            let mut out = Vec::with_capacity(message.len() + template_id.len() + 16);
            out.extend_from_slice(b"arkade-msg-v1:");
            out.extend_from_slice(template_id.as_bytes());
            out.push(0x00);
            out.extend_from_slice(message);
            out
        }
        _ => message.to_vec(),
    }
}

pub fn stack_value_to_bytes(value: &StackValue) -> Vec<u8> {
    match value {
        StackValue::Bytes(v) => v.clone(),
        StackValue::Int(v) => v.to_le_bytes().to_vec(),
        StackValue::Bool(v) => vec![u8::from(*v)],
        StackValue::Symbol(v) => {
            if v.len() % 2 == 0 && !v.is_empty() && v.chars().all(|c| c.is_ascii_hexdigit()) {
                return hex::decode(v).unwrap_or_else(|_| v.as_bytes().to_vec());
            }
            v.as_bytes().to_vec()
        }
    }
}
