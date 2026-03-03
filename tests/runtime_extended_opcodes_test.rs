use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};
use secp256k1::{Scalar, Secp256k1};
use sha2::{Digest, Sha256};

#[test]
fn streaming_sha256_opcodes_runtime() {
    let data = b"alpha".to_vec();
    let chunk = b"beta".to_vec();
    let last = b"gamma".to_vec();

    let init = Sha256::digest(&data).to_vec();
    let updated = Sha256::digest([init.as_slice(), chunk.as_slice()].concat()).to_vec();
    let final_hash = Sha256::digest([updated.as_slice(), last.as_slice()].concat()).to_vec();

    let mut env = ExecutionEnv::default();
    env.bindings
        .insert("data".to_string(), StackValue::Bytes(data));
    env.bindings
        .insert("chunk".to_string(), StackValue::Bytes(chunk));
    env.bindings
        .insert("last".to_string(), StackValue::Bytes(last));
    env.bindings
        .insert("expected".to_string(), StackValue::Bytes(final_hash));

    let script = vec![
        "<data>".to_string(),
        "OP_SHA256INITIALIZE".to_string(),
        "<chunk>".to_string(),
        "OP_SHA256UPDATE".to_string(),
        "<last>".to_string(),
        "OP_SHA256FINALIZE".to_string(),
        "<expected>".to_string(),
        "OP_EQUAL".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn conversion_opcodes_runtime() {
    let mut env = ExecutionEnv::default();
    env.bindings.insert("n".to_string(), StackValue::Int(42));
    env.bindings.insert(
        "le32".to_string(),
        StackValue::Bytes(vec![0x2a, 0x00, 0x00, 0x00]),
    );
    env.bindings.insert(
        "expected_le64".to_string(),
        StackValue::Bytes(vec![0x2a, 0x00, 0x00, 0x00, 0, 0, 0, 0]),
    );

    let script = vec![
        "<n>".to_string(),
        "OP_SCRIPTNUMTOLE64".to_string(),
        "OP_LE64TOSCRIPTNUM".to_string(),
        "<n>".to_string(),
        "OP_EQUALVERIFY".to_string(),
        "<le32>".to_string(),
        "OP_LE32TOLE64".to_string(),
        "<expected_le64>".to_string(),
        "OP_EQUAL".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn ec_verify_opcodes_runtime() {
    let secp = Secp256k1::new();
    let (_sk, point_p) = ExecutionEnv::derive_keypair_for_label("point_p");

    let scalar_bytes = [7u8; 32];
    let scalar = Scalar::from_be_bytes(scalar_bytes).expect("scalar should parse");
    let point_q_mul = point_p
        .mul_tweak(&secp, &scalar)
        .expect("mul tweak should work");

    let tweak_bytes = [3u8; 32];
    let tweak = Scalar::from_be_bytes(tweak_bytes).expect("scalar should parse");
    let point_q_tweak = point_p
        .add_exp_tweak(&secp, &tweak)
        .expect("add tweak should work");

    let mut env = ExecutionEnv::default();
    env.bindings.insert(
        "p".to_string(),
        StackValue::Bytes(point_p.serialize().to_vec()),
    );
    env.bindings.insert(
        "q_mul".to_string(),
        StackValue::Bytes(point_q_mul.serialize().to_vec()),
    );
    env.bindings.insert(
        "scalar".to_string(),
        StackValue::Bytes(scalar_bytes.to_vec()),
    );
    env.bindings.insert(
        "q_tweak".to_string(),
        StackValue::Bytes(point_q_tweak.serialize().to_vec()),
    );
    env.bindings
        .insert("tweak".to_string(), StackValue::Bytes(tweak_bytes.to_vec()));

    let script = vec![
        "<q_mul>".to_string(),
        "<p>".to_string(),
        "<scalar>".to_string(),
        "OP_ECMULSCALARVERIFY".to_string(),
        "<q_tweak>".to_string(),
        "<tweak>".to_string(),
        "<p>".to_string(),
        "OP_TWEAKVERIFY".to_string(),
        "OP_1".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn checksigadd_runtime() {
    let (_sk, pk) = ExecutionEnv::derive_keypair_for_label("signer");
    let mut env = ExecutionEnv::default();
    let signature = ExecutionEnv::sign_message_for_label("signer", &env.tx_context.tx_hash);

    env.bindings
        .insert("pk".to_string(), StackValue::Bytes(pk.serialize().to_vec()));
    env.bindings
        .insert("sig".to_string(), StackValue::Bytes(signature));

    let script = vec![
        "<sig>".to_string(),
        "OP_0".to_string(),
        "<pk>".to_string(),
        "OP_CHECKSIGADD".to_string(),
        "OP_1".to_string(),
        "OP_EQUAL".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn checksigadd_runtime_invalid_signature() {
    let (_sk, pk) = ExecutionEnv::derive_keypair_for_label("signer");
    let mut env = ExecutionEnv::default();
    let mut signature = ExecutionEnv::sign_message_for_label("signer", &env.tx_context.tx_hash);
    signature[0] ^= 0x55;

    env.bindings
        .insert("pk".to_string(), StackValue::Bytes(pk.serialize().to_vec()));
    env.bindings
        .insert("sig".to_string(), StackValue::Bytes(signature));

    let script = vec![
        "<sig>".to_string(),
        "OP_0".to_string(),
        "<pk>".to_string(),
        "OP_CHECKSIGADD".to_string(),
        "OP_0".to_string(),
        "OP_EQUAL".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn checkmultisig_runtime_real() {
    let (_ska, pka) = ExecutionEnv::derive_keypair_for_label("a");
    let (_skb, pkb) = ExecutionEnv::derive_keypair_for_label("b");
    let env = ExecutionEnv::default();
    let siga = ExecutionEnv::sign_message_for_label("a", &env.tx_context.tx_hash);
    let sigb = ExecutionEnv::sign_message_for_label("b", &env.tx_context.tx_hash);

    let mut runtime_env = ExecutionEnv::default();
    runtime_env
        .bindings
        .insert("a".to_string(), StackValue::Bytes(pka.serialize().to_vec()));
    runtime_env
        .bindings
        .insert("b".to_string(), StackValue::Bytes(pkb.serialize().to_vec()));
    runtime_env
        .bindings
        .insert("siga".to_string(), StackValue::Bytes(siga));
    runtime_env
        .bindings
        .insert("sigb".to_string(), StackValue::Bytes(sigb));

    let script = vec![
        "OP_2".to_string(),
        "<a>".to_string(),
        "<b>".to_string(),
        "OP_2".to_string(),
        "<siga>".to_string(),
        "<sigb>".to_string(),
        "OP_CHECKMULTISIG".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&runtime_env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn checkmultisig_runtime_invalid_signature_fails() {
    let (_ska, pka) = ExecutionEnv::derive_keypair_for_label("a");
    let (_skb, pkb) = ExecutionEnv::derive_keypair_for_label("b");
    let env = ExecutionEnv::default();
    let siga = ExecutionEnv::sign_message_for_label("a", &env.tx_context.tx_hash);
    let mut sigb = ExecutionEnv::sign_message_for_label("b", &env.tx_context.tx_hash);
    sigb[0] ^= 0x01;

    let mut runtime_env = ExecutionEnv::default();
    runtime_env
        .bindings
        .insert("a".to_string(), StackValue::Bytes(pka.serialize().to_vec()));
    runtime_env
        .bindings
        .insert("b".to_string(), StackValue::Bytes(pkb.serialize().to_vec()));
    runtime_env
        .bindings
        .insert("siga".to_string(), StackValue::Bytes(siga));
    runtime_env
        .bindings
        .insert("sigb".to_string(), StackValue::Bytes(sigb));

    let script = vec![
        "OP_2".to_string(),
        "<a>".to_string(),
        "<b>".to_string(),
        "OP_2".to_string(),
        "<siga>".to_string(),
        "<sigb>".to_string(),
        "OP_CHECKMULTISIG".to_string(),
        "OP_VERIFY".to_string(),
        "OP_1".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&runtime_env);
    assert_eq!(result.outcome, VmOutcome::ScriptFalse);
}
