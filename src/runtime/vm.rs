#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use sha2::{Digest, Sha256};
use tracing::{debug, error, info};

use crate::runtime::dispatcher::{DispatchOutcome, OpcodeDispatcher};
use crate::runtime::env::{stack_value_to_bytes, ExecutionEnv};
use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::stack::Stack;
use crate::runtime::telemetry::{
    PolicyCounters, RuntimeOptionsSnapshot, StepStatus, StepTelemetry,
};
use crate::runtime::value::StackValue;

#[cfg(not(target_arch = "wasm32"))]
fn timing_start() -> Instant {
    Instant::now()
}

#[cfg(target_arch = "wasm32")]
fn timing_start() {}

#[cfg(not(target_arch = "wasm32"))]
fn timing_elapsed_nanos(started: &Instant) -> u64 {
    started.elapsed().as_nanos() as u64
}

#[cfg(target_arch = "wasm32")]
fn timing_elapsed_nanos(_started: &()) -> u64 {
    0
}

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
    pub trace_version: String,
    pub trace_id: String,
    pub seed: Option<u64>,
    pub runtime_options: RuntimeOptionsSnapshot,
    pub policy_counters: PolicyCounters,
    pub elapsed_nanos: u64,
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
    pub policy_counters: PolicyCounters,
    pub opcode_budget_used: usize,
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
            policy_counters: PolicyCounters::default(),
            opcode_budget_used: 0,
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
            policy_counters: PolicyCounters::default(),
            opcode_budget_used: 0,
        }
    }

    pub fn reset(&mut self) {
        self.ip = 0;
        self.halted = false;
        self.last_opcode = None;
        self.result = None;
        self.telemetry.clear();
        self.stack = Stack::default();
        self.policy_counters = PolicyCounters::default();
        self.opcode_budget_used = 0;
    }

    pub fn run(&mut self, env: &ExecutionEnv) -> VmRunResult {
        info!(
            target: "arkade_runtime",
            script_len = self.script.len(),
            "vm.run started"
        );

        let started = timing_start();

        if let Some(max_script_len) = env.runtime_policy.max_script_len {
            if self.script.len() > max_script_len {
                let err = RuntimeError::new(
                    RuntimeErrorCode::PolicyViolation,
                    format!(
                        "script length {} exceeds max_script_len {}",
                        self.script.len(),
                        max_script_len
                    ),
                );
                self.halted = true;
                self.result = Some(VmOutcome::RuntimeError(err));
            }
        }

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

        self.build_run_result(env, outcome, timing_elapsed_nanos(&started))
    }

    pub fn step(&mut self, env: &ExecutionEnv) -> Result<(), RuntimeError> {
        if self.halted {
            return Ok(());
        }

        if self.ip >= self.script.len() {
            self.finalize_on_end_of_script(env)?;
            return Ok(());
        }

        if let Some(max_steps) = env.runtime_policy.max_steps {
            if self.policy_counters.steps >= max_steps {
                return Err(RuntimeError::new(
                    RuntimeErrorCode::PolicyViolation,
                    format!("max_steps limit reached at {}", self.policy_counters.steps),
                ));
            }
        }

        let ip_before = self.ip;
        let token = self.script[self.ip].clone();
        let stack_before = self.stack.snapshot_main();
        let step_started = timing_start();
        let mut status = StepStatus::Continue;

        if token.starts_with("OP_") {
            if let Some(allowed) = &env.runtime_policy.allowed_opcodes {
                if !allowed.contains(&token) {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::PolicyViolation,
                        format!("opcode '{token}' is not allowed by policy"),
                    ));
                }
            }

            if let Some(max_budget) = env.runtime_policy.max_opcode_budget {
                if self.opcode_budget_used >= max_budget {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::PolicyViolation,
                        format!(
                            "opcode budget exceeded: used={}, max={max_budget}",
                            self.opcode_budget_used
                        ),
                    ));
                }
            }

            self.last_opcode = Some(token.clone());
            self.opcode_budget_used += 1;
            self.policy_counters.opcode_steps += 1;
            *self
                .policy_counters
                .opcode_counts
                .entry(token.clone())
                .or_insert(0) += 1;

            let dispatch = OpcodeDispatcher::dispatch(&token, self, env)
                .map_err(|err| err.with_context(ip_before, token.clone()))?;
            match dispatch {
                DispatchOutcome::Advance => {
                    if token == "OP_IF" {
                        self.policy_counters.conditional_jumps += 1;
                        self.policy_counters
                            .branch_path
                            .push(format!("if@{ip_before}:then"));
                    }
                    self.ip += 1;
                }
                DispatchOutcome::Jump(target) => {
                    if token == "OP_IF" {
                        self.policy_counters.conditional_jumps += 1;
                        self.policy_counters
                            .branch_path
                            .push(format!("if@{ip_before}:else"));
                    } else if token == "OP_ELSE" {
                        self.policy_counters
                            .branch_path
                            .push(format!("else@{ip_before}:jump"));
                    }
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
            self.finalize_on_end_of_script(env)?;
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
        if let Some(max_growth) = env.runtime_policy.max_stack_growth_per_step {
            let growth = stack_after.len().saturating_sub(stack_before.len());
            if growth > max_growth {
                return Err(RuntimeError::new(
                    RuntimeErrorCode::PolicyViolation,
                    format!(
                        "stack growth {} exceeds per-step limit {} at ip {}",
                        growth, max_growth, ip_before
                    ),
                ));
            }
        }

        if let Some(max_depth) = env.runtime_policy.max_main_stack_depth {
            if self.stack.len_main() > max_depth {
                return Err(RuntimeError::new(
                    RuntimeErrorCode::PolicyViolation,
                    format!(
                        "main stack depth {} exceeds policy max {}",
                        self.stack.len_main(),
                        max_depth
                    ),
                ));
            }
        }

        if let Some(max_depth) = env.runtime_policy.max_alt_stack_depth {
            if self.stack.len_alt() > max_depth {
                return Err(RuntimeError::new(
                    RuntimeErrorCode::PolicyViolation,
                    format!(
                        "alt stack depth {} exceeds policy max {}",
                        self.stack.len_alt(),
                        max_depth
                    ),
                ));
            }
        }

        self.policy_counters.steps += 1;
        self.telemetry.push(StepTelemetry {
            step_id: self.policy_counters.steps,
            ip: ip_before,
            token: token.clone(),
            elapsed_nanos: timing_elapsed_nanos(&step_started),
            stack_before: stack_before.clone(),
            stack_after: stack_after.clone(),
            status: status.clone(),
            source_span: None,
            policy_steps: self.policy_counters.steps,
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

    fn finalize_on_end_of_script(&mut self, env: &ExecutionEnv) -> Result<(), RuntimeError> {
        self.halted = true;
        let truth = match self.stack.peek_main() {
            Some(value) => value.as_bool_with_strict(env.strict_types)?,
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

    fn build_run_result(
        &self,
        env: &ExecutionEnv,
        outcome: VmOutcome,
        elapsed_nanos: u64,
    ) -> VmRunResult {
        VmRunResult {
            outcome,
            final_main_stack: self.stack.snapshot_main(),
            final_alt_stack: self.stack.snapshot_alt(),
            telemetry: self.telemetry.clone(),
            trace_version: "v1".to_string(),
            trace_id: compute_trace_id(&self.script, env),
            seed: env.seed,
            runtime_options: RuntimeOptionsSnapshot {
                strict_placeholders: env.strict_placeholders,
                strict_types: env.strict_types,
                strict_bindings: env.strict_bindings,
                execution_mode: env.execution_mode.as_str().to_string(),
                runtime_policy: env.runtime_policy.clone(),
            },
            policy_counters: self.policy_counters.clone(),
            elapsed_nanos,
        }
    }
}

fn compute_trace_id(script: &[String], env: &ExecutionEnv) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"arkade-trace-v1");

    for token in script {
        hasher.update(token.as_bytes());
        hasher.update([0x00]);
    }

    let mut keys = env.bindings.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        hasher.update(key.as_bytes());
        hasher.update([0x00]);
        if let Some(value) = env.bindings.get(&key) {
            hasher.update(stack_value_to_bytes(value));
        }
        hasher.update([0x00]);
    }

    hasher.update(&env.tx_context.tx_hash);
    hasher.update(env.tx_context.version.to_le_bytes());
    hasher.update(env.tx_context.locktime.to_le_bytes());
    hasher.update(env.tx_context.weight.to_le_bytes());
    hasher.update((env.tx_context.current_input_index as u64).to_le_bytes());

    hasher.update([u8::from(env.strict_placeholders)]);
    hasher.update([u8::from(env.strict_types)]);
    hasher.update([u8::from(env.strict_bindings)]);
    hasher.update(env.execution_mode.as_str().as_bytes());

    if let Some(max_steps) = env.runtime_policy.max_steps {
        hasher.update(max_steps.to_le_bytes());
    }
    if let Some(max_script_len) = env.runtime_policy.max_script_len {
        hasher.update(max_script_len.to_le_bytes());
    }
    if let Some(seed) = env.seed {
        hasher.update(seed.to_le_bytes());
    }

    hex::encode(hasher.finalize())
}
