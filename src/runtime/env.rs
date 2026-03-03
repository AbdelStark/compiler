use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use secp256k1::ecdsa::Signature as EcdsaSignature;
#[cfg(not(target_arch = "wasm32"))]
use secp256k1::schnorr::Signature as SchnorrSignature;
#[cfg(not(target_arch = "wasm32"))]
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};

use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::value::StackValue;

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

#[derive(Debug, Clone)]
pub struct AssetEntry {
    pub txid: Vec<u8>,
    pub gidx: u16,
    pub amount: i64,
    pub data: Vec<u8>,
    pub control: Vec<u8>,
    pub metadata_hash: Vec<u8>,
    pub asset_id: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TxInput {
    pub value: i64,
    pub script_pubkey: Vec<u8>,
    pub sequence: i64,
    pub outpoint: Vec<u8>,
    pub issuance: Vec<u8>,
    pub assets: Vec<AssetEntry>,
}

#[derive(Debug, Clone)]
pub struct TxOutput {
    pub value: i64,
    pub script_pubkey: Vec<u8>,
    pub nonce: Vec<u8>,
    pub assets: Vec<AssetEntry>,
}

#[derive(Debug, Clone)]
pub struct AssetGroup {
    pub txid: Vec<u8>,
    pub gidx: u16,
    pub sum_inputs: i64,
    pub sum_outputs: i64,
    pub num_inputs: i64,
    pub num_outputs: i64,
    pub control: Vec<u8>,
    pub metadata_hash: Vec<u8>,
    pub asset_id: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TxContext {
    pub tx_hash: Vec<u8>,
    pub version: i64,
    pub locktime: i64,
    pub weight: i64,
    pub current_input_index: usize,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
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

#[derive(Clone)]
pub struct ExecutionEnv {
    pub bindings: HashMap<String, StackValue>,
    pub strict_placeholders: bool,
    pub tx_context: TxContext,
}

impl ExecutionEnv {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            strict_placeholders: false,
            tx_context: TxContext::default(),
        }
    }

    pub fn resolve_placeholder(&self, key: &str) -> Result<StackValue, RuntimeError> {
        if let Some(v) = self.bindings.get(key) {
            return Ok(v.clone());
        }

        if self.strict_placeholders {
            return Err(RuntimeError::new(
                RuntimeErrorCode::MissingBinding,
                format!("missing binding for placeholder '{key}'"),
            ));
        }

        Ok(StackValue::Symbol(key.to_string()))
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
        let secp = Secp256k1::signing_only();
        let (secret, _) = Self::derive_keypair_for_label(label);
        let digest = Sha256::digest(message);
        let msg = Message::from_digest(digest.into());
        secp.sign_ecdsa(msg, &secret).serialize_der().to_vec()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn sign_message_for_label(label: &str, message: &[u8]) -> Vec<u8> {
        let (_secret, public) = Self::derive_keypair_for_label(label);
        Self::sign_message_for_pubkey_bytes(&public.serialize(), message)
    }

    fn signature_message(&self, message: Option<&StackValue>) -> Vec<u8> {
        message
            .map(stack_value_to_bytes)
            .unwrap_or_else(|| self.tx_context.tx_hash.clone())
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
