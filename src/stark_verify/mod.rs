#![cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use cairo_air::utils::{deserialize_proof_from_file, get_verification_output, ProofFormat};
use cairo_air::verifier::verify_cairo;
use cairo_air::PreProcessedTraceVariant;
use serde::{Deserialize, Serialize};
use stwo::core::vcs::blake2_merkle::{Blake2sMerkleChannel, Blake2sMerkleHasher};
use stwo::core::vcs::poseidon252_merkle::{Poseidon252MerkleChannel, Poseidon252MerkleHasher};

const CHANNEL_BLAKE2S: &str = "blake2s";
const CHANNEL_POSEIDON252: &str = "poseidon252";
const FORMAT_JSON: &str = "json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StarkPublicInputs {
    pub program_hash: String,
    pub output: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StarkVerificationKey {
    pub channel_hash: String,
    pub include_all_preprocessed_columns: bool,
    pub proof_format: String,
}

impl Default for StarkVerificationKey {
    fn default() -> Self {
        Self {
            channel_hash: CHANNEL_BLAKE2S.to_string(),
            include_all_preprocessed_columns: false,
            proof_format: FORMAT_JSON.to_string(),
        }
    }
}

pub fn materialize_artifacts_from_proof(
    proof_path: &Path,
    public_inputs_path: &Path,
    verification_key_path: &Path,
) -> Result<()> {
    let verification_key = StarkVerificationKey::default();
    let public_inputs = read_public_inputs_from_proof(proof_path, &verification_key)?;

    write_json(public_inputs_path, &public_inputs)?;
    write_json(verification_key_path, &verification_key)?;
    Ok(())
}

pub fn verify_stark_proof_from_files(
    proof_path: &Path,
    public_inputs_path: &Path,
    verification_key_path: &Path,
) -> Result<()> {
    let expected_inputs: StarkPublicInputs = read_json(public_inputs_path)?;
    let verification_key: StarkVerificationKey = read_json(verification_key_path)?;
    let observed_inputs = verify_and_extract_public_inputs(proof_path, &verification_key)?;

    if expected_inputs != observed_inputs {
        bail!(
            "public inputs mismatch: expected {:?}, observed {:?}",
            expected_inputs,
            observed_inputs
        );
    }

    Ok(())
}

pub fn execute_op_stark_verify(
    proof_path: &Path,
    public_inputs_path: &Path,
    verification_key_path: &Path,
) -> Result<()> {
    verify_stark_proof_from_files(proof_path, public_inputs_path, verification_key_path)
}

fn verify_and_extract_public_inputs(
    proof_path: &Path,
    verification_key: &StarkVerificationKey,
) -> Result<StarkPublicInputs> {
    let proof_format = parse_proof_format(&verification_key.proof_format)?;
    let preprocessed_trace = preprocessed_trace_variant(verification_key);

    match verification_key.channel_hash.as_str() {
        CHANNEL_BLAKE2S => {
            let proof =
                deserialize_proof_from_file::<Blake2sMerkleHasher>(proof_path, proof_format)
                    .with_context(|| {
                        format!("failed to deserialize proof at {}", proof_path.display())
                    })?;
            let public_inputs = public_inputs_from_verification_output(get_verification_output(
                &proof.claim.public_data.public_memory,
            ));
            verify_cairo::<Blake2sMerkleChannel>(proof, preprocessed_trace)
                .context("STWO verification failed for blake2s channel")?;
            Ok(public_inputs)
        }
        CHANNEL_POSEIDON252 => {
            let proof =
                deserialize_proof_from_file::<Poseidon252MerkleHasher>(proof_path, proof_format)
                    .with_context(|| {
                        format!("failed to deserialize proof at {}", proof_path.display())
                    })?;
            let public_inputs = public_inputs_from_verification_output(get_verification_output(
                &proof.claim.public_data.public_memory,
            ));
            verify_cairo::<Poseidon252MerkleChannel>(proof, preprocessed_trace)
                .context("STWO verification failed for poseidon252 channel")?;
            Ok(public_inputs)
        }
        unsupported => bail!("unsupported STWO channel hash '{}'", unsupported),
    }
}

fn read_public_inputs_from_proof(
    proof_path: &Path,
    verification_key: &StarkVerificationKey,
) -> Result<StarkPublicInputs> {
    let proof_format = parse_proof_format(&verification_key.proof_format)?;

    match verification_key.channel_hash.as_str() {
        CHANNEL_BLAKE2S => {
            let proof =
                deserialize_proof_from_file::<Blake2sMerkleHasher>(proof_path, proof_format)
                    .with_context(|| {
                        format!("failed to deserialize proof at {}", proof_path.display())
                    })?;
            Ok(public_inputs_from_verification_output(
                get_verification_output(&proof.claim.public_data.public_memory),
            ))
        }
        CHANNEL_POSEIDON252 => {
            let proof =
                deserialize_proof_from_file::<Poseidon252MerkleHasher>(proof_path, proof_format)
                    .with_context(|| {
                        format!("failed to deserialize proof at {}", proof_path.display())
                    })?;
            Ok(public_inputs_from_verification_output(
                get_verification_output(&proof.claim.public_data.public_memory),
            ))
        }
        unsupported => bail!("unsupported STWO channel hash '{}'", unsupported),
    }
}

fn parse_proof_format(format: &str) -> Result<ProofFormat> {
    match format {
        "json" => Ok(ProofFormat::Json),
        "binary" => Ok(ProofFormat::Binary),
        "cairo_serde" => Ok(ProofFormat::CairoSerde),
        "extended_binary" => {
            bail!(
                "extended_binary proof format is not supported by the pinned Cairo1 verifier stack"
            )
        }
        _ => bail!("unsupported proof format '{}'", format),
    }
}

fn preprocessed_trace_variant(verification_key: &StarkVerificationKey) -> PreProcessedTraceVariant {
    let _ = verification_key;
    PreProcessedTraceVariant::Canonical
}

fn public_inputs_from_verification_output(
    output: cairo_air::utils::VerificationOutput,
) -> StarkPublicInputs {
    StarkPublicInputs {
        program_hash: field_element_to_hex(&output.program_hash),
        output: output.output.iter().map(field_element_to_hex).collect(),
    }
}

fn field_element_to_hex(field: &starknet_ff::FieldElement) -> String {
    format!("0x{field:x}")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let file =
        File::open(path).with_context(|| format!("failed to open JSON file {}", path.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("failed to parse JSON file {}", path.display()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value).context("failed to serialize JSON")?;
    std::fs::write(path, json)
        .with_context(|| format!("failed to write JSON file {}", path.display()))
}

pub fn default_output_paths(base_dir: &Path) -> (PathBuf, PathBuf) {
    (
        base_dir.join("public_inputs.json"),
        base_dir.join("verification_key.json"),
    )
}
