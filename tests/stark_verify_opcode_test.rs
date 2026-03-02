use arkade_compiler::compile;
use arkade_compiler::opcodes::OP_STARK_VERIFY;

#[test]
fn test_verify_stark_proof_emits_opcode_and_requirement() {
    let code = r#"
        options {
            server = serverKey;
            exit = 144;
        }

        contract StarkVerifier(pubkey owner) {
            function unlock(signature ownerSig, bytes proof, bytes publicInputs, bytes verificationKey) {
                require(checkSig(ownerSig, owner));
                require(verifyStarkProof(proof, publicInputs, verificationKey));
            }
        }
    "#;

    let result = compile(code);
    assert!(result.is_ok(), "Compilation failed: {:?}", result.err());

    let output = result.unwrap();
    let server = output
        .functions
        .iter()
        .find(|f| f.name == "unlock" && f.server_variant)
        .expect("missing server variant for unlock");

    assert!(
        server.require.iter().any(|r| r.req_type == "starkVerify"),
        "missing starkVerify requirement: {:?}",
        server.require
    );

    let asm = server.asm.join(" ");
    assert!(
        asm.contains(OP_STARK_VERIFY),
        "missing {OP_STARK_VERIFY} in asm: {asm}"
    );

    let expected = "<proof> <publicInputs> <verificationKey> OP_STARK_VERIFY";
    assert!(
        asm.contains(expected),
        "expected stark verify stack order `{expected}`, got: {asm}"
    );
}
