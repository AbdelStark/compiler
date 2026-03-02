use std::fs;
use std::path::PathBuf;
use std::process::Command;

use arkade_compiler::compile;
use arkade_compiler::opcodes::OP_STARK_VERIFY;
use arkade_compiler::stark_verify::execute_op_stark_verify;
use serde_json::Value;

#[test]
fn test_stark_verify_end_to_end() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let prove_status = Command::new("bash")
        .arg("scripts/generate_proof.sh")
        .current_dir(&root)
        .status()
        .expect("failed to execute scripts/generate_proof.sh");
    assert!(prove_status.success(), "generate_proof.sh failed");

    let contract = include_str!("../examples/stark_verifier.ark");
    let compiled = compile(contract).expect("failed to compile stark_verifier.ark");
    let unlock_server = compiled
        .functions
        .iter()
        .find(|f| f.name == "unlock" && f.server_variant)
        .expect("missing unlock server variant");

    let asm = unlock_server.asm.join(" ");
    assert!(
        asm.contains(OP_STARK_VERIFY),
        "missing {OP_STARK_VERIFY} in unlock asm: {asm}"
    );

    let artifacts_dir = root.join("examples/stark_demo/artifacts");
    let proof_path = artifacts_dir.join("proof.json");
    let public_inputs_path = artifacts_dir.join("public_inputs.json");
    let verification_key_path = artifacts_dir.join("verification_key.json");

    execute_op_stark_verify(&proof_path, &public_inputs_path, &verification_key_path)
        .expect("valid proof should be accepted");

    let tampered_dir = tempfile::tempdir().expect("failed to create tempdir for tampered proof");
    let tampered_proof_path = tampered_dir.path().join("proof_tampered.json");

    let mut proof_json: Value =
        serde_json::from_str(&fs::read_to_string(&proof_path).expect("failed to read proof"))
            .expect("failed to parse proof json");
    let interaction_pow = proof_json
        .get("interaction_pow")
        .and_then(Value::as_u64)
        .expect("proof missing numeric interaction_pow");
    proof_json["interaction_pow"] = Value::from(interaction_pow + 1);

    fs::write(
        &tampered_proof_path,
        serde_json::to_string_pretty(&proof_json).expect("failed to serialize tampered proof"),
    )
    .expect("failed to write tampered proof");

    let err = execute_op_stark_verify(
        &tampered_proof_path,
        &public_inputs_path,
        &verification_key_path,
    )
    .expect_err("tampered proof should be rejected");

    let err_msg = format!("{err:#}");
    assert!(
        err_msg.contains("Proof of work") || err_msg.contains("STWO verification failed"),
        "unexpected error for tampered proof: {err_msg}"
    );
}
