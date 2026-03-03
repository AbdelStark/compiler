use secp256k1::{PublicKey, Scalar, Secp256k1};
use sha2::{Digest, Sha256};

use crate::runtime::env::{stack_value_to_bytes, AssetEntry, ExecutionEnv, TxInput, TxOutput};
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

enum IoSource {
    Input,
    Output,
}

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
                vm.stack.push_main(StackValue::Bool(!value.as_bool()?))?;
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
                vm.stack
                    .push_main(StackValue::Int(left.as_i64()? / divisor))?;
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
                let digest = Sha256::digest(stack_value_to_bytes(&value));
                vm.stack.push_main(StackValue::Bytes(digest.to_vec()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_SHA256INITIALIZE" => {
                let data = vm.stack.pop_main()?;
                let digest = Sha256::digest(stack_value_to_bytes(&data));
                vm.stack.push_main(StackValue::Bytes(digest.to_vec()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_SHA256UPDATE" => {
                let chunk = vm.stack.pop_main()?;
                let ctx = vm.stack.pop_main()?;
                let mut data = stack_value_to_bytes(&ctx);
                data.extend_from_slice(&stack_value_to_bytes(&chunk));
                let digest = Sha256::digest(data);
                vm.stack.push_main(StackValue::Bytes(digest.to_vec()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_SHA256FINALIZE" => {
                let chunk = vm.stack.pop_main()?;
                let ctx = vm.stack.pop_main()?;
                let mut data = stack_value_to_bytes(&ctx);
                data.extend_from_slice(&stack_value_to_bytes(&chunk));
                let digest = Sha256::digest(data);
                vm.stack.push_main(StackValue::Bytes(digest.to_vec()))?;
                Ok(DispatchOutcome::Advance)
            }

            "OP_CHECKSIG" => {
                let signature = vm.stack.pop_main()?;
                let pubkey = vm.stack.pop_main()?;
                vm.stack.push_main(StackValue::Bool(
                    env.verify_signature(&pubkey, &signature, None),
                ))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_CHECKSIGVERIFY" => {
                let signature = vm.stack.pop_main()?;
                let pubkey = vm.stack.pop_main()?;
                if env.verify_signature(&pubkey, &signature, None) {
                    Ok(DispatchOutcome::Advance)
                } else {
                    Ok(DispatchOutcome::HaltFalse)
                }
            }
            "OP_CHECKSIGFROMSTACK" => {
                let signature = vm.stack.pop_main()?;
                let pubkey = vm.stack.pop_main()?;
                let message = vm.stack.pop_main()?;
                vm.stack.push_main(StackValue::Bool(env.verify_signature(
                    &pubkey,
                    &signature,
                    Some(&message),
                )))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_CHECKSIGFROMSTACKVERIFY" => {
                let signature = vm.stack.pop_main()?;
                let pubkey = vm.stack.pop_main()?;
                let message = vm.stack.pop_main()?;
                if env.verify_signature(&pubkey, &signature, Some(&message)) {
                    Ok(DispatchOutcome::Advance)
                } else {
                    Ok(DispatchOutcome::HaltFalse)
                }
            }
            "OP_CHECKSIGADD" => {
                let pubkey = vm.stack.pop_main()?;
                let n = vm.stack.pop_main()?.as_i64()?;
                let signature = vm.stack.pop_main()?;
                let ok = env.verify_signature(&pubkey, &signature, None);
                vm.stack
                    .push_main(StackValue::Int(n.saturating_add(i64::from(ok))))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_CHECKMULTISIG" => {
                let mut signatures: Vec<StackValue> = Vec::new();
                let sig_count = loop {
                    let value = vm.stack.pop_main()?;
                    if let Some(count) = Self::try_count(&value) {
                        break count;
                    }
                    signatures.push(value);
                };
                signatures.reverse();

                let mut pubkeys: Vec<StackValue> = Vec::new();
                let key_count = loop {
                    let value = vm.stack.pop_main()?;
                    if let Some(count) = Self::try_count(&value) {
                        break count;
                    }
                    pubkeys.push(value);
                };
                pubkeys.reverse();

                let ok = key_count >= sig_count
                    && signatures.len() == sig_count
                    && pubkeys.len() == key_count
                    && env.verify_multisig(&pubkeys, &signatures);
                vm.stack.push_main(StackValue::Bool(ok))?;
                Ok(DispatchOutcome::Advance)
            }

            "OP_SCRIPTNUMTOLE64" => {
                let v = vm.stack.pop_main()?.as_i64()?;
                vm.stack
                    .push_main(StackValue::Bytes(v.to_le_bytes().to_vec()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_LE64TOSCRIPTNUM" => {
                let bytes = stack_value_to_bytes(&vm.stack.pop_main()?);
                if bytes.len() > 8 {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        "OP_LE64TOSCRIPTNUM expects at most 8 bytes",
                    ));
                }
                let mut out = [0u8; 8];
                out[..bytes.len()].copy_from_slice(&bytes);
                vm.stack
                    .push_main(StackValue::Int(i64::from_le_bytes(out)))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_LE32TOLE64" => {
                let bytes = stack_value_to_bytes(&vm.stack.pop_main()?);
                if bytes.len() > 4 {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        "OP_LE32TOLE64 expects at most 4 bytes",
                    ));
                }
                let mut out = [0u8; 8];
                out[..bytes.len()].copy_from_slice(&bytes);
                vm.stack.push_main(StackValue::Bytes(out.to_vec()))?;
                Ok(DispatchOutcome::Advance)
            }

            "OP_ECMULSCALARVERIFY" => {
                let scalar_value = vm.stack.pop_main()?;
                let point_p_value = vm.stack.pop_main()?;
                let point_q_value = vm.stack.pop_main()?;

                let ok = Self::verify_ec_mul_scalar(&point_p_value, &point_q_value, &scalar_value);
                if ok {
                    Ok(DispatchOutcome::Advance)
                } else {
                    Ok(DispatchOutcome::HaltFalse)
                }
            }
            "OP_TWEAKVERIFY" => {
                let point_p_value = vm.stack.pop_main()?;
                let tweak_value = vm.stack.pop_main()?;
                let point_q_value = vm.stack.pop_main()?;

                let ok = Self::verify_tweak(&point_p_value, &point_q_value, &tweak_value);
                if ok {
                    Ok(DispatchOutcome::Advance)
                } else {
                    Ok(DispatchOutcome::HaltFalse)
                }
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

            "OP_TXHASH" => {
                vm.stack
                    .push_main(StackValue::Bytes(env.tx_context.tx_hash.clone()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_TXWEIGHT" => {
                vm.stack.push_main(StackValue::Int(env.tx_context.weight))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTVERSION" => {
                vm.stack
                    .push_main(StackValue::Int(env.tx_context.version))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTLOCKTIME" => {
                vm.stack
                    .push_main(StackValue::Int(env.tx_context.locktime))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTNUMINPUTS" => {
                vm.stack
                    .push_main(StackValue::Int(env.tx_context.inputs.len() as i64))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTNUMOUTPUTS" => {
                vm.stack
                    .push_main(StackValue::Int(env.tx_context.outputs.len() as i64))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_PUSHCURRENTINPUTINDEX" => {
                vm.stack
                    .push_main(StackValue::Int(env.tx_context.current_input_index as i64))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INPUTBYTECODE" => {
                let input = env
                    .tx_context
                    .input_at(env.tx_context.current_input_index)
                    .ok_or_else(|| {
                        RuntimeError::new(
                            RuntimeErrorCode::InvalidNumericEncoding,
                            "current input index is out of bounds",
                        )
                    })?;
                vm.stack
                    .push_main(StackValue::Bytes(input.script_pubkey.clone()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INPUTVALUE" => {
                let input = env
                    .tx_context
                    .input_at(env.tx_context.current_input_index)
                    .ok_or_else(|| {
                        RuntimeError::new(
                            RuntimeErrorCode::InvalidNumericEncoding,
                            "current input index is out of bounds",
                        )
                    })?;
                vm.stack.push_main(StackValue::Int(input.value))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INPUTSEQUENCE" => {
                let input = env
                    .tx_context
                    .input_at(env.tx_context.current_input_index)
                    .ok_or_else(|| {
                        RuntimeError::new(
                            RuntimeErrorCode::InvalidNumericEncoding,
                            "current input index is out of bounds",
                        )
                    })?;
                vm.stack.push_main(StackValue::Int(input.sequence))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INPUTOUTPOINT" => {
                let input = env
                    .tx_context
                    .input_at(env.tx_context.current_input_index)
                    .ok_or_else(|| {
                        RuntimeError::new(
                            RuntimeErrorCode::InvalidNumericEncoding,
                            "current input index is out of bounds",
                        )
                    })?;
                vm.stack
                    .push_main(StackValue::Bytes(input.outpoint.clone()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTINPUTSCRIPTPUBKEY" => {
                let index = Self::pop_index(vm)?;
                let input = env.tx_context.input_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("input index {index} is out of bounds"),
                    )
                })?;
                vm.stack
                    .push_main(StackValue::Bytes(input.script_pubkey.clone()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTINPUTVALUE" => {
                let index = Self::pop_index(vm)?;
                let input = env.tx_context.input_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("input index {index} is out of bounds"),
                    )
                })?;
                vm.stack.push_main(StackValue::Int(input.value))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTINPUTSEQUENCE" => {
                let index = Self::pop_index(vm)?;
                let input = env.tx_context.input_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("input index {index} is out of bounds"),
                    )
                })?;
                vm.stack.push_main(StackValue::Int(input.sequence))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTINPUTOUTPOINT" => {
                let index = Self::pop_index(vm)?;
                let input = env.tx_context.input_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("input index {index} is out of bounds"),
                    )
                })?;
                vm.stack
                    .push_main(StackValue::Bytes(input.outpoint.clone()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTINPUTISSUANCE" => {
                let index = Self::pop_index(vm)?;
                let input = env.tx_context.input_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("input index {index} is out of bounds"),
                    )
                })?;
                vm.stack
                    .push_main(StackValue::Bytes(input.issuance.clone()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTOUTPUTVALUE" => {
                let index = Self::pop_index(vm)?;
                let output = env.tx_context.output_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("output index {index} is out of bounds"),
                    )
                })?;
                vm.stack.push_main(StackValue::Int(output.value))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTOUTPUTSCRIPTPUBKEY" => {
                let index = Self::pop_index(vm)?;
                let output = env.tx_context.output_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("output index {index} is out of bounds"),
                    )
                })?;
                vm.stack
                    .push_main(StackValue::Bytes(output.script_pubkey.clone()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTOUTPUTNONCE" => {
                let index = Self::pop_index(vm)?;
                let output = env.tx_context.output_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("output index {index} is out of bounds"),
                    )
                })?;
                vm.stack
                    .push_main(StackValue::Bytes(output.nonce.clone()))?;
                Ok(DispatchOutcome::Advance)
            }

            "OP_FINDASSETGROUPBYASSETID" => {
                let gidx = Self::pop_u16(vm)?;
                let txid = Self::pop_txid(vm)?;
                let group_index = env.tx_context.find_group_index(&txid, gidx);
                vm.stack.push_main(StackValue::Int(group_index))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTNUMASSETGROUPS" => {
                vm.stack
                    .push_main(StackValue::Int(env.tx_context.asset_groups.len() as i64))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTASSETGROUPSUM" => {
                let source = Self::pop_i64(vm)?;
                let group_index = Self::pop_index(vm)?;
                let group = env.tx_context.group_at(group_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("asset group index {group_index} is out of bounds"),
                    )
                })?;
                let value = if source == 0 {
                    group.sum_inputs
                } else {
                    group.sum_outputs
                };
                vm.stack.push_main(StackValue::Int(value))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTASSETGROUPNUM" => {
                let source = Self::pop_i64(vm)?;
                let group_index = Self::pop_index(vm)?;
                let group = env.tx_context.group_at(group_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("asset group index {group_index} is out of bounds"),
                    )
                })?;
                let value = if source == 0 {
                    group.num_inputs
                } else {
                    group.num_outputs
                };
                vm.stack.push_main(StackValue::Int(value))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTASSETGROUPASSETID" => {
                let group_index = Self::pop_index(vm)?;
                let group = env.tx_context.group_at(group_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("asset group index {group_index} is out of bounds"),
                    )
                })?;
                vm.stack.push_main(StackValue::Bytes(group.txid.clone()))?;
                vm.stack.push_main(StackValue::Int(group.gidx as i64))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTASSETGROUPCTRL" => {
                let group_index = Self::pop_index(vm)?;
                let group = env.tx_context.group_at(group_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("asset group index {group_index} is out of bounds"),
                    )
                })?;
                vm.stack
                    .push_main(StackValue::Bytes(group.control.clone()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTASSETGROUPMETADATAHASH" => {
                let group_index = Self::pop_index(vm)?;
                let group = env.tx_context.group_at(group_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("asset group index {group_index} is out of bounds"),
                    )
                })?;
                vm.stack
                    .push_main(StackValue::Bytes(group.metadata_hash.clone()))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTASSETGROUP" => {
                let source = Self::pop_i64(vm)?;
                let io_index = Self::pop_index(vm)?;
                let group_index = Self::pop_index(vm)?;
                let group = env.tx_context.group_at(group_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("asset group index {group_index} is out of bounds"),
                    )
                })?;

                let asset = if source == 0 {
                    let input = env.tx_context.input_at(io_index).ok_or_else(|| {
                        RuntimeError::new(
                            RuntimeErrorCode::InvalidNumericEncoding,
                            format!("input index {io_index} is out of bounds"),
                        )
                    })?;
                    Self::find_asset_by_group(&input.assets, group.txid.as_slice(), group.gidx)
                } else {
                    let output = env.tx_context.output_at(io_index).ok_or_else(|| {
                        RuntimeError::new(
                            RuntimeErrorCode::InvalidNumericEncoding,
                            format!("output index {io_index} is out of bounds"),
                        )
                    })?;
                    Self::find_asset_by_group(&output.assets, group.txid.as_slice(), group.gidx)
                }
                .ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        "group/io combination has no matching asset",
                    )
                })?;

                vm.stack.push_main(StackValue::Bytes(asset.txid.clone()))?;
                vm.stack.push_main(StackValue::Int(asset.gidx as i64))?;
                vm.stack.push_main(StackValue::Int(asset.amount))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTINASSETLOOKUP" => {
                Self::asset_lookup(vm, env, IoSource::Input)?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTOUTASSETLOOKUP" => {
                Self::asset_lookup(vm, env, IoSource::Output)?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTINASSETCOUNT" => {
                let index = Self::pop_index(vm)?;
                let input = env.tx_context.input_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("input index {index} is out of bounds"),
                    )
                })?;
                vm.stack
                    .push_main(StackValue::Int(input.assets.len() as i64))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTOUTASSETCOUNT" => {
                let index = Self::pop_index(vm)?;
                let output = env.tx_context.output_at(index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("output index {index} is out of bounds"),
                    )
                })?;
                vm.stack
                    .push_main(StackValue::Int(output.assets.len() as i64))?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTINASSETAT" => {
                Self::inspect_asset_at(vm, env, IoSource::Input)?;
                Ok(DispatchOutcome::Advance)
            }
            "OP_INSPECTOUTASSETAT" => {
                Self::inspect_asset_at(vm, env, IoSource::Output)?;
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
        vm.stack
            .push_main(StackValue::Int(f(left.as_i64()?, right.as_i64()?)))?;
        Ok(DispatchOutcome::Advance)
    }

    fn binary_cmp(
        vm: &mut VMState,
        f: impl Fn(i64, i64) -> bool,
    ) -> Result<DispatchOutcome, RuntimeError> {
        let right = vm.stack.pop_main()?;
        let left = vm.stack.pop_main()?;
        vm.stack
            .push_main(StackValue::Bool(f(left.as_i64()?, right.as_i64()?)))?;
        Ok(DispatchOutcome::Advance)
    }

    fn pop_i64(vm: &mut VMState) -> Result<i64, RuntimeError> {
        vm.stack.pop_main()?.as_i64()
    }

    fn pop_index(vm: &mut VMState) -> Result<usize, RuntimeError> {
        let v = Self::pop_i64(vm)?;
        if v < 0 {
            return Err(RuntimeError::new(
                RuntimeErrorCode::InvalidNumericEncoding,
                format!("negative index {v} is invalid"),
            ));
        }
        Ok(v as usize)
    }

    fn pop_u16(vm: &mut VMState) -> Result<u16, RuntimeError> {
        let v = Self::pop_i64(vm)?;
        if !(0..=u16::MAX as i64).contains(&v) {
            return Err(RuntimeError::new(
                RuntimeErrorCode::InvalidNumericEncoding,
                format!("value {v} cannot fit in u16"),
            ));
        }
        Ok(v as u16)
    }

    fn pop_txid(vm: &mut VMState) -> Result<Vec<u8>, RuntimeError> {
        let raw = stack_value_to_bytes(&vm.stack.pop_main()?);
        if raw.len() == 32 {
            Ok(raw)
        } else {
            Ok(Sha256::digest(raw).to_vec())
        }
    }

    fn verify_ec_mul_scalar(
        point_p: &StackValue,
        point_q: &StackValue,
        scalar: &StackValue,
    ) -> bool {
        let secp = Secp256k1::verification_only();

        let pk_p = match PublicKey::from_slice(&stack_value_to_bytes(point_p)) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let pk_q = match PublicKey::from_slice(&stack_value_to_bytes(point_q)) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let scalar_bytes = scalar_to_32bytes(scalar);
        let tweak = match Scalar::from_be_bytes(scalar_bytes) {
            Ok(v) => v,
            Err(_) => return false,
        };

        match pk_p.mul_tweak(&secp, &tweak) {
            Ok(tweaked) => tweaked == pk_q,
            Err(_) => false,
        }
    }

    fn verify_tweak(point_p: &StackValue, point_q: &StackValue, tweak: &StackValue) -> bool {
        let secp = Secp256k1::verification_only();

        let pk_p = match PublicKey::from_slice(&stack_value_to_bytes(point_p)) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let pk_q = match PublicKey::from_slice(&stack_value_to_bytes(point_q)) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let tweak_bytes = scalar_to_32bytes(tweak);
        let scalar = match Scalar::from_be_bytes(tweak_bytes) {
            Ok(v) => v,
            Err(_) => return false,
        };

        match pk_p.add_exp_tweak(&secp, &scalar) {
            Ok(tweaked) => tweaked == pk_q,
            Err(_) => false,
        }
    }

    fn asset_lookup(
        vm: &mut VMState,
        env: &ExecutionEnv,
        source: IoSource,
    ) -> Result<(), RuntimeError> {
        let gidx = Self::pop_u16(vm)?;
        let txid = Self::pop_txid(vm)?;
        let io_index = Self::pop_index(vm)?;

        let asset_index = match source {
            IoSource::Input => {
                let input = env.tx_context.input_at(io_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("input index {io_index} is out of bounds"),
                    )
                })?;
                Self::lookup_asset_index(&input.assets, txid.as_slice(), gidx)
            }
            IoSource::Output => {
                let output = env.tx_context.output_at(io_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("output index {io_index} is out of bounds"),
                    )
                })?;
                Self::lookup_asset_index(&output.assets, txid.as_slice(), gidx)
            }
        };

        vm.stack.push_main(StackValue::Int(asset_index))?;
        Ok(())
    }

    fn inspect_asset_at(
        vm: &mut VMState,
        env: &ExecutionEnv,
        source: IoSource,
    ) -> Result<(), RuntimeError> {
        let asset_index = Self::pop_index(vm)?;
        let io_index = Self::pop_index(vm)?;

        let asset = match source {
            IoSource::Input => {
                let input = env.tx_context.input_at(io_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("input index {io_index} is out of bounds"),
                    )
                })?;
                input.assets.get(asset_index)
            }
            IoSource::Output => {
                let output = env.tx_context.output_at(io_index).ok_or_else(|| {
                    RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        format!("output index {io_index} is out of bounds"),
                    )
                })?;
                output.assets.get(asset_index)
            }
        }
        .ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::InvalidNumericEncoding,
                format!("asset index {asset_index} is out of bounds"),
            )
        })?;

        vm.stack.push_main(StackValue::Bytes(asset.txid.clone()))?;
        vm.stack.push_main(StackValue::Int(asset.gidx as i64))?;
        vm.stack.push_main(StackValue::Int(asset.amount))?;
        Ok(())
    }

    fn lookup_asset_index(assets: &[AssetEntry], txid: &[u8], gidx: u16) -> i64 {
        assets
            .iter()
            .position(|asset| asset.txid == txid && asset.gidx == gidx)
            .map(|idx| idx as i64)
            .unwrap_or(-1)
    }

    fn try_count(value: &StackValue) -> Option<usize> {
        match value.as_i64() {
            Ok(v) if v >= 0 => Some(v as usize),
            _ => None,
        }
    }

    fn find_asset_by_group<'a>(
        assets: &'a [AssetEntry],
        txid: &[u8],
        gidx: u16,
    ) -> Option<&'a AssetEntry> {
        assets
            .iter()
            .find(|asset| asset.txid == txid && asset.gidx == gidx)
    }
}

fn scalar_to_32bytes(value: &StackValue) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = stack_value_to_bytes(value);
    if bytes.len() >= 32 {
        out.copy_from_slice(&bytes[bytes.len() - 32..]);
    } else {
        out[32 - bytes.len()..].copy_from_slice(&bytes);
    }
    out
}
