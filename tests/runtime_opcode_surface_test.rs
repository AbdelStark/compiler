use std::collections::BTreeSet;

use arkade_compiler::runtime::env::ExecutionEnv;
use arkade_compiler::runtime::error::RuntimeErrorCode;
use arkade_compiler::runtime::stack::Stack;
use arkade_compiler::runtime::value::StackValue;
use arkade_compiler::runtime::vm::{VMState, VmOutcome};

fn all_declared_opcodes() -> Vec<String> {
    let mut set = BTreeSet::new();
    for line in include_str!("../src/opcodes/mod.rs").lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub const OP_") {
            continue;
        }
        let Some(rhs) = trimmed.split('=').nth(1) else {
            continue;
        };
        let op = rhs.trim().trim_end_matches(';').trim().trim_matches('"');
        if op.starts_with("OP_") {
            set.insert(op.to_string());
        }
    }
    set.into_iter().collect()
}

fn seeded_stack() -> Stack {
    let mut values = Vec::new();
    for i in 0..32 {
        values.push(StackValue::Int(i));
    }
    values.push(StackValue::Bool(true));
    values.push(StackValue::Bytes(vec![0x01; 32]));
    values.push(StackValue::Bytes(vec![0x02; 33]));
    values.push(StackValue::Bytes(vec![0x03; 64]));
    Stack::with_main(values, 10_000)
}

#[test]
fn every_declared_opcode_is_handled_by_dispatcher() {
    let opcodes = all_declared_opcodes();
    assert!(
        !opcodes.is_empty(),
        "no opcodes parsed from src/opcodes/mod.rs"
    );

    let env = ExecutionEnv::default();
    let mut unknown: Vec<String> = Vec::new();
    let mut unsupported: Vec<String> = Vec::new();

    for opcode in opcodes {
        let mut vm = VMState::with_stack(vec![opcode.clone()], seeded_stack());
        let result = vm.run(&env);

        if let VmOutcome::RuntimeError(err) = result.outcome {
            if err.code == RuntimeErrorCode::UnknownOpcode {
                unknown.push(opcode.clone());
            }
            if err.code == RuntimeErrorCode::UnsupportedOpcode {
                unsupported.push(opcode.clone());
            }
        }
    }

    assert!(
        unknown.is_empty(),
        "unknown opcode coverage gap: {}",
        unknown.join(", ")
    );
    assert!(
        unsupported.is_empty(),
        "unsupported opcode coverage gap: {}",
        unsupported.join(", ")
    );
}
