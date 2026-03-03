use crate::runtime::value::StackValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    Continue,
    ScriptTrue,
    ScriptFalse,
    RuntimeError,
}

#[derive(Debug, Clone)]
pub struct StepTelemetry {
    pub ip: usize,
    pub token: String,
    pub stack_before: Vec<StackValue>,
    pub stack_after: Vec<StackValue>,
    pub status: StepStatus,
}
