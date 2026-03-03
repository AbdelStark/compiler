use tracing::{debug, error, info};

use crate::runtime::dispatcher::{DispatchOutcome, OpcodeDispatcher};
use crate::runtime::env::ExecutionEnv;
use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::stack::Stack;
use crate::runtime::telemetry::{StepStatus, StepTelemetry};
use crate::runtime::value::StackValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmOutcome {
    ScriptTrue,
    ScriptFalse,
    RuntimeError(RuntimeError),
}

#[derive(Debug, Clone)]
pub struct VmRunResult {
    pub outcome: VmOutcome,
    pub final_main_stack: Vec<StackValue>,
    pub final_alt_stack: Vec<StackValue>,
    pub telemetry: Vec<StepTelemetry>,
}

#[derive(Debug, Clone)]
pub struct VMState {
    pub ip: usize,
    pub script: Vec<String>,
    pub stack: Stack,
    pub halted: bool,
    pub last_opcode: Option<String>,
    pub result: Option<VmOutcome>,
    pub telemetry: Vec<StepTelemetry>,
}

impl VMState {
    pub fn new(script: Vec<String>) -> Self {
        Self {
            ip: 0,
            script,
            stack: Stack::default(),
            halted: false,
            last_opcode: None,
            result: None,
            telemetry: Vec::new(),
        }
    }

    pub fn with_stack(script: Vec<String>, stack: Stack) -> Self {
        Self {
            ip: 0,
            script,
            stack,
            halted: false,
            last_opcode: None,
            result: None,
            telemetry: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.ip = 0;
        self.halted = false;
        self.last_opcode = None;
        self.result = None;
        self.telemetry.clear();
        self.stack = Stack::default();
    }

    pub fn run(&mut self, env: &ExecutionEnv) -> VmRunResult {
        info!(
            target: "arkade_runtime",
            script_len = self.script.len(),
            "vm.run started"
        );

        while !self.halted {
            if let Err(err) = self.step(env) {
                error!(
                    target: "arkade_runtime",
                    ip = self.ip,
                    error = %err,
                    "vm runtime error"
                );
                self.halted = true;
                self.result = Some(VmOutcome::RuntimeError(err));
            }
        }

        let outcome = self.result.clone().unwrap_or(VmOutcome::ScriptFalse);
        info!(
            target: "arkade_runtime",
            ip = self.ip,
            outcome = ?outcome,
            "vm.run finished"
        );

        VmRunResult {
            outcome,
            final_main_stack: self.stack.snapshot_main(),
            final_alt_stack: self.stack.snapshot_alt(),
            telemetry: self.telemetry.clone(),
        }
    }

    pub fn step(&mut self, env: &ExecutionEnv) -> Result<(), RuntimeError> {
        if self.halted {
            return Ok(());
        }

        if self.ip >= self.script.len() {
            self.finalize_on_end_of_script()?;
            return Ok(());
        }

        let ip_before = self.ip;
        let token = self.script[self.ip].clone();
        let stack_before = self.stack.snapshot_main();
        let mut status = StepStatus::Continue;

        if token.starts_with("OP_") {
            self.last_opcode = Some(token.clone());
            let dispatch = OpcodeDispatcher::dispatch(&token, self, env)
                .map_err(|err| err.with_context(ip_before, token.clone()))?;
            match dispatch {
                DispatchOutcome::Advance => {
                    self.ip += 1;
                }
                DispatchOutcome::Jump(target) => {
                    self.ip = target;
                }
                DispatchOutcome::HaltFalse => {
                    self.halted = true;
                    self.result = Some(VmOutcome::ScriptFalse);
                    status = StepStatus::ScriptFalse;
                }
            }
        } else {
            let pushed = Self::parse_push_token(&token, env)?;
            self.stack.push_main(pushed)?;
            self.ip += 1;
        }

        if !self.halted && self.ip >= self.script.len() {
            self.finalize_on_end_of_script()?;
            status = match self.result {
                Some(VmOutcome::ScriptTrue) => StepStatus::ScriptTrue,
                Some(VmOutcome::ScriptFalse) => StepStatus::ScriptFalse,
                Some(VmOutcome::RuntimeError(_)) => StepStatus::RuntimeError,
                None => StepStatus::Continue,
            };
        } else if matches!(self.result, Some(VmOutcome::RuntimeError(_))) {
            status = StepStatus::RuntimeError;
        } else if matches!(self.result, Some(VmOutcome::ScriptFalse)) {
            status = StepStatus::ScriptFalse;
        }

        let stack_after = self.stack.snapshot_main();
        self.telemetry.push(StepTelemetry {
            ip: ip_before,
            token: token.clone(),
            stack_before: stack_before.clone(),
            stack_after: stack_after.clone(),
            status: status.clone(),
        });

        debug!(
            target: "arkade_runtime",
            ip = ip_before,
            token = %token,
            status = ?status,
            stack_before = ?stack_before,
            stack_after = ?stack_after,
            "vm.step"
        );

        Ok(())
    }

    pub fn find_matching_else_or_endif(&self, from_ip: usize) -> Result<usize, RuntimeError> {
        let mut depth = 0usize;
        let mut i = from_ip + 1;

        while i < self.script.len() {
            match self.script[i].as_str() {
                "OP_IF" => depth += 1,
                "OP_ENDIF" => {
                    if depth == 0 {
                        return Ok(i + 1);
                    }
                    depth -= 1;
                }
                "OP_ELSE" if depth == 0 => return Ok(i + 1),
                _ => {}
            }
            i += 1;
        }

        Err(RuntimeError::new(
            RuntimeErrorCode::UnbalancedConditional,
            "OP_IF has no matching OP_ELSE/OP_ENDIF",
        ))
    }

    pub fn find_matching_endif(&self, from_ip: usize) -> Result<usize, RuntimeError> {
        let mut depth = 0usize;
        let mut i = from_ip + 1;

        while i < self.script.len() {
            match self.script[i].as_str() {
                "OP_IF" => depth += 1,
                "OP_ENDIF" => {
                    if depth == 0 {
                        return Ok(i + 1);
                    }
                    depth -= 1;
                }
                _ => {}
            }
            i += 1;
        }

        Err(RuntimeError::new(
            RuntimeErrorCode::UnbalancedConditional,
            "OP_ELSE has no matching OP_ENDIF",
        ))
    }

    fn finalize_on_end_of_script(&mut self) -> Result<(), RuntimeError> {
        self.halted = true;
        let truth = match self.stack.peek_main() {
            Some(value) => value.as_bool()?,
            None => false,
        };
        self.result = Some(if truth {
            VmOutcome::ScriptTrue
        } else {
            VmOutcome::ScriptFalse
        });
        Ok(())
    }

    fn parse_push_token(token: &str, env: &ExecutionEnv) -> Result<StackValue, RuntimeError> {
        if token.starts_with('<') && token.ends_with('>') {
            let inner = &token[1..token.len() - 1];
            return env.resolve_placeholder(inner);
        }

        if token.eq_ignore_ascii_case("true") {
            return Ok(StackValue::Bool(true));
        }
        if token.eq_ignore_ascii_case("false") {
            return Ok(StackValue::Bool(false));
        }

        if let Ok(v) = token.parse::<i64>() {
            return Ok(StackValue::Int(v));
        }

        if token.len().is_multiple_of(2)
            && !token.is_empty()
            && token.chars().all(|c| c.is_ascii_hexdigit())
        {
            let bytes = hex::decode(token).map_err(|err| {
                RuntimeError::new(
                    RuntimeErrorCode::InvalidNumericEncoding,
                    format!("invalid hex token '{token}': {err}"),
                )
            })?;
            return Ok(StackValue::Bytes(bytes));
        }

        Ok(StackValue::Symbol(token.to_string()))
    }
}
