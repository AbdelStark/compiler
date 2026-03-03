use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::value::StackValue;

pub trait ChecksigProvider {
    fn check_sig(
        &self,
        pubkey: &StackValue,
        signature: &StackValue,
        message: Option<&StackValue>,
    ) -> bool;
}

#[derive(Debug, Default)]
pub struct MockChecksigProvider;

impl ChecksigProvider for MockChecksigProvider {
    fn check_sig(
        &self,
        _pubkey: &StackValue,
        _signature: &StackValue,
        _message: Option<&StackValue>,
    ) -> bool {
        true
    }
}

#[derive(Clone)]
pub struct ExecutionEnv {
    pub bindings: HashMap<String, StackValue>,
    pub strict_placeholders: bool,
    pub checksig: Arc<dyn ChecksigProvider + Send + Sync>,
}

impl ExecutionEnv {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            strict_placeholders: false,
            checksig: Arc::new(MockChecksigProvider),
        }
    }

    pub fn with_binding(mut self, key: impl Into<String>, value: StackValue) -> Self {
        self.bindings.insert(key.into(), value);
        self
    }

    pub fn resolve_placeholder(&self, key: &str) -> Result<StackValue, RuntimeError> {
        if let Some(v) = self.bindings.get(key) {
            return Ok(v.clone());
        }

        if self.strict_placeholders {
            return Err(RuntimeError::new(
                RuntimeErrorCode::MissingBinding,
                format!("missing binding for placeholder '{key}'"),
            ));
        }

        Ok(StackValue::Symbol(key.to_string()))
    }
}

impl Default for ExecutionEnv {
    fn default() -> Self {
        Self::new()
    }
}
