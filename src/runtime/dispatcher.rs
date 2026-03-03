use sha2::{Digest, Sha256};

use crate::runtime::env::ExecutionEnv;
use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::value::StackValue;
use crate::runtime::vm::VMState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    Advance,
    Jump(usize),
    HaltFalse,
}

pub struct OpcodeDispatcher;

impl OpcodeDispatcher {
    pub fn dispatch(
        opcode: &str,
        vm: &mut VMState,
        env: &ExecutionEnv,
    ) -> Result<DispatchOutcome, RuntimeError> {
        match opcode {
            "OP_0" | "OP_FALSE" => {
                vm.stack.push_main(StackValue::Int(0))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_1NEGATE" => {
                vm.stack.push_main(StackValue::Int(-1))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_1" => Self::push_int(vm, 1),
            "OP_2" => Self::push_int(vm, 2),
            "OP_3" => Self::push_int(vm, 3),
            "OP_4" => Self::push_int(vm, 4),
            "OP_5" => Self::push_int(vm, 5),
            "OP_6" => Self::push_int(vm, 6),
            "OP_7" => Self::push_int(vm, 7),
            "OP_8" => Self::push_int(vm, 8),
            "OP_9" => Self::push_int(vm, 9),
            "OP_10" => Self::push_int(vm, 10),
            "OP_11" => Self::push_int(vm, 11),
            "OP_12" => Self::push_int(vm, 12),
            "OP_13" => Self::push_int(vm, 13),
            "OP_14" => Self::push_int(vm, 14),
            "OP_15" => Self::push_int(vm, 15),
            "OP_16" => Self::push_int(vm, 16),
            "OP_DUP" => {
                let top = vm.stack.peek_main().cloned().ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::StackUnderflow,
                        "OP_DUP requires at least one stack item",
                    )
                })?;
                vm.stack.push_main(top)?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_DROP" => {
                vm.stack.pop_main()?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_NIP" => {
                let top = vm.stack.pop_main()?;
                vm.stack.pop_main()?;
                vm.stack.push_main(top)?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_NOT" => {
                let value = vm.stack.pop_main()?;
                let out = !value.as_bool()?;
                vm.stack.push_main(StackValue::Bool(out))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_EQUAL" => {
                let b = vm.stack.pop_main()?;
                let a = vm.stack.pop_main()?;
                vm.stack.push_main(StackValue::Bool(a == b))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_EQUALVERIFY" => {
                let b = vm.stack.pop_main()?;
                let a = vm.stack.pop_main()?;
                if a == b {
                    Ok(DispatchOutcome::Advance)
                } else {
                    Ok(DispatchOutcome::HaltFalse)
                }
            }
            "OP_VERIFY" => {
                let top = vm.stack.pop_main()?;
                if top.as_bool()? {
                    Ok(DispatchOutcome::Advance)
                } else {
                    Ok(DispatchOutcome::HaltFalse)
                }
            }
            "OP_ADD" | "OP_ADD64" => Self::binary_i64(vm, |a, b| a.saturating_add(b)),
            "OP_SUB" | "OP_SUB64" => Self::binary_i64(vm, |a, b| a.saturating_sub(b)),
            "OP_MUL" | "OP_MUL64" => Self::binary_i64(vm, |a, b| a.saturating_mul(b)),
            "OP_DIV" | "OP_DIV64" => {
                let right = vm.stack.pop_main()?;
                let left = vm.stack.pop_main()?;
                let divisor = right.as_i64()?;
                if divisor == 0 {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::DivisionByZero,
                        "OP_DIV64 divisor is zero",
                    ));
                }
                let quotient = left.as_i64()? / divisor;
                vm.stack.push_main(StackValue::Int(quotient))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_GREATERTHAN" | "OP_GREATERTHAN64" => Self::binary_cmp(vm, |a, b| a > b),
            "OP_GREATERTHANOREQUAL" | "OP_GREATERTHANOREQUAL64" => {
                Self::binary_cmp(vm, |a, b| a >= b)
            }
            "OP_LESSTHAN" | "OP_LESSTHAN64" => Self::binary_cmp(vm, |a, b| a < b),
            "OP_LESSTHANOREQUAL" | "OP_LESSTHANOREQUAL64" => Self::binary_cmp(vm, |a, b| a <= b),
            "OP_NEG64" => {
                let value = vm.stack.pop_main()?;
                vm.stack
                    .push_main(StackValue::Int(value.as_i64()?.saturating_neg()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_SHA256" => {
                let value = vm.stack.pop_main()?;
                let digest = Sha256::digest(value.to_bytes());
                vm.stack.push_main(StackValue::Bytes(digest.to_vec()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_CHECKSIG" => {
                let signature = vm.stack.pop_main()?;
                let pubkey = vm.stack.pop_main()?;
                let ok = env.checksig.check_sig(&pubkey, &signature, None);
                vm.stack.push_main(StackValue::Bool(ok))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_CHECKSIGVERIFY" => {
                let signature = vm.stack.pop_main()?;
                let pubkey = vm.stack.pop_main()?;
                let ok = env.checksig.check_sig(&pubkey, &signature, None);
                if ok {
                    Ok(DispatchOutcome::Advance)
                } else {
                    Ok(DispatchOutcome::HaltFalse)
                }
            }
            "OP_CHECKSIGFROMSTACK" => {
                let signature = vm.stack.pop_main()?;
                let pubkey = vm.stack.pop_main()?;
                let message = vm.stack.pop_main()?;
                let ok = env.checksig.check_sig(&pubkey, &signature, Some(&message));
                vm.stack.push_main(StackValue::Bool(ok))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_CHECKSIGFROMSTACKVERIFY" => {
                let signature = vm.stack.pop_main()?;
                let pubkey = vm.stack.pop_main()?;
                let message = vm.stack.pop_main()?;
                let ok = env.checksig.check_sig(&pubkey, &signature, Some(&message));
                if ok {
                    Ok(DispatchOutcome::Advance)
                } else {
                    Ok(DispatchOutcome::HaltFalse)
                }
            }
            "OP_CHECKMULTISIG" => {
                let mut signatures: Vec<StackValue> = Vec::new();
                let signature_count = loop {
                    let value = vm.stack.pop_main()?;
                    match value {
                        StackValue::Int(v) if v >= 0 => break v as usize,
                        other => signatures.push(other),
                    }
                };
                signatures.reverse();

                let mut pubkeys: Vec<StackValue> = Vec::new();
                let pubkey_count = loop {
                    let value = vm.stack.pop_main()?;
                    match value {
                        StackValue::Int(v) if v >= 0 => break v as usize,
                        other => pubkeys.push(other),
                    }
                };
                pubkeys.reverse();

                let lengths_match =
                    signature_count == signatures.len() && pubkey_count == pubkeys.len();
                let count_ok = signature_count <= pubkey_count;
                let sigs_ok = signatures
                    .iter()
                    .zip(pubkeys.iter())
                    .all(|(sig, key)| env.checksig.check_sig(key, sig, None));

                vm.stack
                    .push_main(StackValue::Bool(lengths_match && count_ok && sigs_ok))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_CHECKLOCKTIMEVERIFY" | "OP_CHECKSEQUENCEVERIFY" => {
                let _ = vm.stack.peek_main().ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::StackUnderflow,
                        format!("{opcode} requires one stack item"),
                    )
                })?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_IF" => {
                let condition = vm.stack.pop_main()?.as_bool()?;
                if condition {
                    Ok(DispatchOutcome::Advance)
                } else {
                    let jump = vm.find_matching_else_or_endif(vm.ip)?;
                    Ok(DispatchOutcome::Jump(jump))
                }
            }
            "OP_ELSE" => {
                let jump = vm.find_matching_endif(vm.ip)?;
                Ok(DispatchOutcome::Jump(jump))
            }
            "OP_ENDIF" => Ok(DispatchOutcome::Advance),

            "OP_SHA256INITIALIZE"
            | "OP_SHA256UPDATE"
            | "OP_SHA256FINALIZE"
            | "OP_LE64TOSCRIPTNUM"
            | "OP_SCRIPTNUMTOLE64"
            | "OP_LE32TOLE64"
            | "OP_ECMULSCALARVERIFY"
            | "OP_TWEAKVERIFY"
            | "OP_CHECKSIGADD"
            | "OP_TXHASH"
            | "OP_TXWEIGHT"
            | "OP_INSPECTASSETGROUP"
            | "OP_INSPECTASSETGROUPNUM"
            | "OP_INSPECTASSETGROUPSUM"
            | "OP_INSPECTNUMASSETGROUPS"
            | "OP_FINDASSETGROUPBYASSETID"
            | "OP_INSPECTASSETGROUPCTRL"
            | "OP_INSPECTASSETGROUPMETADATAHASH"
            | "OP_INSPECTASSETGROUPASSETID"
            | "OP_PUSHCURRENTINPUTINDEX"
            | "OP_INSPECTINPUTSCRIPTPUBKEY"
            | "OP_INSPECTINPUTVALUE"
            | "OP_INSPECTINPUTSEQUENCE"
            | "OP_INSPECTINPUTOUTPOINT"
            | "OP_INSPECTINASSETLOOKUP"
            | "OP_INSPECTOUTASSETLOOKUP"
            | "OP_INSPECTINASSETCOUNT"
            | "OP_INSPECTOUTASSETCOUNT"
            | "OP_INSPECTINASSETAT"
            | "OP_INSPECTOUTASSETAT"
            | "OP_INSPECTVERSION"
            | "OP_INSPECTLOCKTIME"
            | "OP_INSPECTNUMINPUTS"
            | "OP_INSPECTNUMOUTPUTS"
            | "OP_INSPECTINPUTISSUANCE"
            | "OP_INSPECTOUTPUTVALUE"
            | "OP_INSPECTOUTPUTSCRIPTPUBKEY"
            | "OP_INSPECTOUTPUTNONCE"
            | "OP_INPUTBYTECODE"
            | "OP_INPUTVALUE"
            | "OP_INPUTSEQUENCE"
            | "OP_INPUTOUTPOINT" => Err(RuntimeError::new(
                RuntimeErrorCode::UnsupportedOpcode,
                format!("opcode {opcode} is not implemented in runtime v1"),
            )),

            _ => Err(RuntimeError::new(
                RuntimeErrorCode::UnknownOpcode,
                format!("unknown opcode {opcode}"),
            )),
        }
    }

    fn push_int(vm: &mut VMState, value: i64) -> Result<DispatchOutcome, RuntimeError> {
        vm.stack.push_main(StackValue::Int(value))?;
        Ok(DispatchOutcome::Advance)
    }

    fn binary_i64(
        vm: &mut VMState,
        f: impl Fn(i64, i64) -> i64,
    ) -> Result<DispatchOutcome, RuntimeError> {
        let right = vm.stack.pop_main()?;
        let left = vm.stack.pop_main()?;
        let output = f(left.as_i64()?, right.as_i64()?);
        vm.stack.push_main(StackValue::Int(output))?;
        Ok(DispatchOutcome::Advance)
    }

    fn binary_cmp(
        vm: &mut VMState,
        f: impl Fn(i64, i64) -> bool,
    ) -> Result<DispatchOutcome, RuntimeError> {
        let right = vm.stack.pop_main()?;
        let left = vm.stack.pop_main()?;
        let output = f(left.as_i64()?, right.as_i64()?);
        vm.stack.push_main(StackValue::Bool(output))?;
        Ok(DispatchOutcome::Advance)
    }
}
