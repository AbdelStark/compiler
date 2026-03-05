use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeErrorCode {
    StackUnderflow,
    StackOverflow,
    InvalidNumericEncoding,
    DivisionByZero,
    InvalidBooleanEncoding,
    InvalidBinding,
    UnknownOpcode,
    UnsupportedOpcode,
    UnbalancedConditional,
    MissingBinding,
    PolicyViolation,
    FunctionNotFound,
    Io,
    Json,
    Debugger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeError {
    pub code: RuntimeErrorCode,
    pub message: String,
    pub ip: Option<usize>,
    pub opcode: Option<String>,
}

impl RuntimeError {
    pub fn new(code: RuntimeErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            ip: None,
            opcode: None,
        }
    }

    pub fn with_context(mut self, ip: usize, opcode: impl Into<String>) -> Self {
        self.ip = Some(ip);
        self.opcode = Some(opcode.into());
        self
    }
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (self.ip, self.opcode.as_ref()) {
            (Some(ip), Some(opcode)) => {
                write!(
                    f,
                    "{:?} at ip {} ({}): {}",
                    self.code, ip, opcode, self.message
                )
            }
            (Some(ip), None) => write!(f, "{:?} at ip {}: {}", self.code, ip, self.message),
            _ => write!(f, "{:?}: {}", self.code, self.message),
        }
    }
}

impl Error for RuntimeError {}
