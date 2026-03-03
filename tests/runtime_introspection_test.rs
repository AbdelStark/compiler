use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};

#[test]
fn tx_introspection_smoke() {
    let script = vec![
        "OP_TXHASH".to_string(),
        "OP_DUP".to_string(),
        "OP_TXHASH".to_string(),
        "OP_EQUALVERIFY".to_string(),
        "OP_INSPECTNUMINPUTS".to_string(),
        "OP_0".to_string(),
        "OP_GREATERTHAN".to_string(),
        "OP_VERIFY".to_string(),
        "OP_INSPECTNUMOUTPUTS".to_string(),
        "OP_0".to_string(),
        "OP_GREATERTHAN".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&ExecutionEnv::default());
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn input_output_inspection_smoke() {
    let script = vec![
        "OP_0".to_string(),
        "OP_INSPECTINPUTVALUE".to_string(),
        "OP_0".to_string(),
        "OP_GREATERTHAN".to_string(),
        "OP_VERIFY".to_string(),
        "OP_0".to_string(),
        "OP_INSPECTOUTPUTVALUE".to_string(),
        "OP_0".to_string(),
        "OP_GREATERTHAN".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&ExecutionEnv::default());
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn asset_lookup_and_asset_at_roundtrip() {
    let mut env = ExecutionEnv::default();
    let group = env.tx_context.asset_groups[0].clone();

    env.bindings
        .insert("asset_txid".to_string(), StackValue::Bytes(group.txid));
    env.bindings
        .insert("asset_gidx".to_string(), StackValue::Int(group.gidx as i64));

    let script = vec![
        "OP_0".to_string(),
        "<asset_txid>".to_string(),
        "<asset_gidx>".to_string(),
        "OP_INSPECTOUTASSETLOOKUP".to_string(),
        "OP_DUP".to_string(),
        "OP_1NEGATE".to_string(),
        "OP_EQUAL".to_string(),
        "OP_NOT".to_string(),
        "OP_VERIFY".to_string(),
        "OP_0".to_string(),
        "OP_0".to_string(),
        "OP_INSPECTOUTASSETAT".to_string(),
        "OP_NIP".to_string(),
        "OP_NIP".to_string(),
        "OP_EQUAL".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}

#[test]
fn group_introspection_ops_smoke() {
    let mut env = ExecutionEnv::default();
    env.bindings.insert("group".to_string(), StackValue::Int(0));

    let script = vec![
        "<group>".to_string(),
        "OP_0".to_string(),
        "OP_INSPECTASSETGROUPSUM".to_string(),
        "<group>".to_string(),
        "OP_1".to_string(),
        "OP_INSPECTASSETGROUPSUM".to_string(),
        "OP_GREATERTHAN".to_string(),
        "OP_VERIFY".to_string(),
        "<group>".to_string(),
        "OP_INSPECTASSETGROUPASSETID".to_string(),
        "OP_DROP".to_string(),
        "OP_TXHASH".to_string(),
        "OP_EQUAL".to_string(),
        "OP_NOT".to_string(),
    ];

    let mut vm = VMState::new(script);
    let result = vm.run(&env);
    assert_eq!(result.outcome, VmOutcome::ScriptTrue);
}
