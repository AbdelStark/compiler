use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::runtime::env::RuntimePolicy;
use crate::runtime::value::StackValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Continue,
    ScriptTrue,
    ScriptFalse,
    RuntimeError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeOptionsSnapshot {
    pub strict_placeholders: bool,
    pub strict_types: bool,
    pub strict_bindings: bool,
    pub execution_mode: String,
    pub runtime_policy: RuntimePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyCounters {
    pub steps: usize,
    pub opcode_steps: usize,
    pub conditional_jumps: usize,
    pub branch_path: Vec<String>,
    pub opcode_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTelemetry {
    pub step_id: usize,
    pub ip: usize,
    pub token: String,
    pub elapsed_nanos: u64,
    pub stack_before: Vec<StackValue>,
    pub stack_after: Vec<StackValue>,
    pub status: StepStatus,
    pub source_span: Option<SourceSpan>,
    pub policy_steps: usize,
}
