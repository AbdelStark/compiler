use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};
use sha2::{Digest, Sha256};

#[test]
fn sha256_equalverify_success() {
    let preimage = b"hello".to_vec();
    let hash = Sha256::digest(&preimage).to_vec();
    let script = vec![
        "<preimage>".to_string(),
        "OP_SHA256".to_string(),
        "<hash>".to_string(),
        "OP_EQUALVERIFY".to_string(),
        "OP_1".to_string(),
    ];

    let mut env = ExecutionEnv::default();
    env.bindings
        .insert("preimage".to_string(), StackValue::Bytes(preimage));
    env.bindings
        .insert("hash".to_string(), StackValue::Bytes(hash));

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn checksig_real_success() {
    let script = vec![
        "<pubkey>".to_string(),
        "<signature>".to_string(),
        "OP_CHECKSIG".to_string(),
    ];

    let mut env = ExecutionEnv::default();
    let (_sk, pk) = ExecutionEnv::derive_keypair_for_label("pubkey");
    let signature = ExecutionEnv::sign_message_for_label("pubkey", &env.tx_context.tx_hash);

    env.bindings.insert(
        "pubkey".to_string(),
        StackValue::Bytes(pk.serialize().to_vec()),
    );
    env.bindings
        .insert("signature".to_string(), StackValue::Bytes(signature));

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn checksigverify_real_failure_is_script_false() {
    let script = vec![
        "<pubkey>".to_string(),
        "<signature>".to_string(),
        "OP_CHECKSIGVERIFY".to_string(),
        "OP_1".to_string(),
    ];

    let mut env = ExecutionEnv::default();
    let (_sk, pk) = ExecutionEnv::derive_keypair_for_label("pubkey");
    let mut signature = ExecutionEnv::sign_message_for_label("pubkey", &env.tx_context.tx_hash);
    if let Some(first) = signature.first_mut() {
        *first ^= 0x01;
    }

    env.bindings.insert(
        "pubkey".to_string(),
        StackValue::Bytes(pk.serialize().to_vec()),
    );
    env.bindings
        .insert("signature".to_string(), StackValue::Bytes(signature));

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptFalse);
}

#[test]
fn checksigfromstack_real_success() {
    let script = vec![
        "<msg>".to_string(),
        "<pubkey>".to_string(),
        "<signature>".to_string(),
        "OP_CHECKSIGFROMSTACKVERIFY".to_string(),
        "OP_1".to_string(),
    ];

    let mut env = ExecutionEnv::default();
    let (_sk, pk) = ExecutionEnv::derive_keypair_for_label("pubkey");
    let msg = b"checksigfromstack-message".to_vec();
    let signature = ExecutionEnv::sign_message_for_label("pubkey", &msg);

    env.bindings
        .insert("msg".to_string(), StackValue::Bytes(msg));
    env.bindings.insert(
        "pubkey".to_string(),
        StackValue::Bytes(pk.serialize().to_vec()),
    );
    env.bindings
        .insert("signature".to_string(), StackValue::Bytes(signature));

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}
