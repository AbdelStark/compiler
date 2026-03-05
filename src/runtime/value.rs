use crate::runtime::error::{RuntimeError, RuntimeErrorCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackValue {
    Int(i64),
    Bool(bool),
    Bytes(Vec<u8>),
    Symbol(String),
}

impl StackValue {
    pub fn as_i64(&self) -> Result<i64, RuntimeError> {
        self.as_i64_with_strict(false)
    }

    pub fn as_i64_with_strict(&self, strict_types: bool) -> Result<i64, RuntimeError> {
        match self {
            Self::Int(v) => Ok(*v),
            Self::Bool(v) => {
                if strict_types {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        "strict_types: bool cannot be coerced to i64",
                    ));
                }
                Ok(if *v { 1 } else { 0 })
            }
            Self::Symbol(v) => v.parse::<i64>().map_err(|_| {
                RuntimeError::new(
                    RuntimeErrorCode::InvalidNumericEncoding,
                    format!("cannot parse symbol '{v}' as i64"),
                )
            }),
            Self::Bytes(v) => {
                if strict_types {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        "strict_types: byte buffer cannot be coerced to i64",
                    ));
                }
                if v.is_empty() {
                    return Ok(0);
                }
                if v.len() > 8 {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::InvalidNumericEncoding,
                        "byte buffer longer than 8 bytes cannot be converted to i64",
                    ));
                }

                let mut out = [0u8; 8];
                out[..v.len()].copy_from_slice(v);
                Ok(i64::from_le_bytes(out))
            }
        }
    }

    pub fn as_bool(&self) -> Result<bool, RuntimeError> {
        self.as_bool_with_strict(false)
    }

    pub fn as_bool_with_strict(&self, strict_types: bool) -> Result<bool, RuntimeError> {
        match self {
            Self::Bool(v) => Ok(*v),
            Self::Int(v) => {
                if strict_types {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::InvalidBooleanEncoding,
                        "strict_types: i64 cannot be coerced to bool",
                    ));
                }
                Ok(*v != 0)
            }
            Self::Bytes(v) => {
                if strict_types {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::InvalidBooleanEncoding,
                        "strict_types: byte buffer cannot be coerced to bool",
                    ));
                }
                Ok(v.iter().any(|b| *b != 0))
            }
            Self::Symbol(v) => {
                if strict_types {
                    return Err(RuntimeError::new(
                        RuntimeErrorCode::InvalidBooleanEncoding,
                        "strict_types: symbol cannot be coerced to bool",
                    ));
                }
                if v.eq_ignore_ascii_case("true") {
                    Ok(true)
                } else {
                    Ok(!v.eq_ignore_ascii_case("false") && !v.is_empty())
                }
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Bytes(v) => v.clone(),
            Self::Int(v) => v.to_le_bytes().to_vec(),
            Self::Bool(v) => vec![u8::from(*v)],
            Self::Symbol(v) => v.as_bytes().to_vec(),
        }
    }

    pub fn display_compact(&self) -> String {
        match self {
            Self::Int(v) => v.to_string(),
            Self::Bool(v) => v.to_string(),
            Self::Bytes(v) => format!("0x{}", hex::encode(v)),
            Self::Symbol(v) => format!("<{v}>"),
        }
    }
}
