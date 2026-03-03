use std::sync::Arc;

use arkade_compiler::runtime::env::{ChecksigProvider, ExecutionEnv};
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
fn checksig_mock_success() {
    let script = vec![
        "<pubkey>".to_string(),
        "<signature>".to_string(),
        "OP_CHECKSIG".to_string(),
    ];

    let mut env = ExecutionEnv::default();
    env.bindings
        .insert("pubkey".to_string(), StackValue::Symbol("pk".to_string()));
    env.bindings.insert(
        "signature".to_string(),
        StackValue::Symbol("sig".to_string()),
    );

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[derive(Debug, Default)]
struct DenyChecksigProvider;

impl ChecksigProvider for DenyChecksigProvider {
    fn check_sig(
        &self,
        _pubkey: &StackValue,
        _signature: &StackValue,
        _message: Option<&StackValue>,
    ) -> bool {
        false
    }
}

#[test]
fn checksigverify_failure_is_script_false() {
    let script = vec![
        "<pubkey>".to_string(),
        "<signature>".to_string(),
        "OP_CHECKSIGVERIFY".to_string(),
        "OP_1".to_string(),
    ];

    let mut env = ExecutionEnv::default();
    env.checksig = Arc::new(DenyChecksigProvider);
    env.bindings
        .insert("pubkey".to_string(), StackValue::Symbol("pk".to_string()));
    env.bindings.insert(
        "signature".to_string(),
        StackValue::Symbol("sig".to_string()),
    );

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptFalse);
}
