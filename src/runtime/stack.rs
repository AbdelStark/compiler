use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use crate::runtime::value::StackValue;

#[derive(Debug, Clone)]
pub struct Stack {
    main: Vec<StackValue>,
    alt: Vec<StackValue>,
    max_depth: usize,
}

impl Stack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            main: Vec::new(),
            alt: Vec::new(),
            max_depth,
        }
    }

    pub fn with_main(main: Vec<StackValue>, max_depth: usize) -> Self {
        Self {
            main,
            alt: Vec::new(),
            max_depth,
        }
    }

    pub fn push_main(&mut self, value: StackValue) -> Result<(), RuntimeError> {
        if self.main.len() >= self.max_depth {
            return Err(RuntimeError::new(
                RuntimeErrorCode::StackOverflow,
                format!("stack exceeded max depth {}", self.max_depth),
            ));
        }
        self.main.push(value);
        Ok(())
    }

    pub fn pop_main(&mut self) -> Result<StackValue, RuntimeError> {
        self.main.pop().ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::StackUnderflow,
                "cannot pop from empty main stack",
            )
        })
    }

    pub fn peek_main(&self) -> Option<&StackValue> {
        self.main.last()
    }

    pub fn len_main(&self) -> usize {
        self.main.len()
    }

    pub fn len_alt(&self) -> usize {
        self.alt.len()
    }

    pub fn clear_main(&mut self) {
        self.main.clear();
    }

    pub fn snapshot_main(&self) -> Vec<StackValue> {
        self.main.clone()
    }

    pub fn snapshot_alt(&self) -> Vec<StackValue> {
        self.alt.clone()
    }

    pub fn push_alt(&mut self, value: StackValue) -> Result<(), RuntimeError> {
        if self.alt.len() >= self.max_depth {
            return Err(RuntimeError::new(
                RuntimeErrorCode::StackOverflow,
                format!("alt stack exceeded max depth {}", self.max_depth),
            ));
        }
        self.alt.push(value);
        Ok(())
    }

    pub fn pop_alt(&mut self) -> Result<StackValue, RuntimeError> {
        self.alt.pop().ok_or_else(|| {
            RuntimeError::new(
                RuntimeErrorCode::StackUnderflow,
                "cannot pop from empty alt stack",
            )
        })
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new(1_000)
    }
}
