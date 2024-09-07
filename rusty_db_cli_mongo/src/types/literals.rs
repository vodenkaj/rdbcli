use std::{
    num::{ParseFloatError, ParseIntError},
    str::FromStr,
};

use rusty_db_cli_derive_internals::{TryFrom, WithType};
use serde::{Serialize, Serializer};

use super::expressions::Callee;
use crate::standard_library::{TypeInfo, Typed};

#[derive(Debug, Clone, TryFrom, WithType)]
pub enum Literal {
    String(String),
    Number(Number),
    Bool(bool),
    Null(Null),
}

impl Typed for String {
    fn get_type_info(&self) -> TypeInfo {
        TypeInfo {
            name: "String".to_string(),
            methods: vec![],
        }
    }
}

impl Typed for bool {
    fn get_type_info(&self) -> TypeInfo {
        TypeInfo {
            name: "Bool".to_string(),
            methods: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    F64(f64),
    I64(i64),
    I32(i32),
}

impl ToString for Number {
    fn to_string(&self) -> String {
        match self {
            Number::F64(n) => n.to_string(),
            Number::I64(n) => n.to_string(),
            Number::I32(n) => n.to_string(),
        }
    }
}

impl From<Number> for serde_json::Number {
    fn from(val: Number) -> Self {
        match val {
            Number::F64(f) => serde_json::Number::from_f64(f).unwrap(),
            Number::I64(i) => serde_json::Number::from(i),
            Number::I32(i) => serde_json::Number::from(i),
        }
    }
}

impl From<Number> for u64 {
    fn from(val: Number) -> Self {
        match val {
            Number::F64(v) => v as u64,
            Number::I64(v) => v as u64,
            Number::I32(v) => v as u64,
        }
    }
}

impl From<Number> for i64 {
    fn from(val: Number) -> Self {
        match val {
            Number::F64(v) => v as i64,
            Number::I64(v) => v,
            Number::I32(v) => v as i64,
        }
    }
}

impl Serialize for Number {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Number::F64(f) => f.serialize(serializer),
            Number::I64(i) => i.serialize(serializer),
            Number::I32(i) => i.serialize(serializer),
        }
    }
}

#[derive(Debug)]
pub enum NumberParseError {
    FloatError(ParseFloatError),
    IntError(ParseIntError),
}

impl From<ParseIntError> for NumberParseError {
    fn from(e: ParseIntError) -> Self {
        Self::IntError(e)
    }
}

impl From<ParseFloatError> for NumberParseError {
    fn from(e: ParseFloatError) -> Self {
        Self::FloatError(e)
    }
}

impl FromStr for Number {
    type Err = NumberParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains('.') {
            match s.parse::<f64>() {
                Ok(v) => Ok(Number::F64(v)),
                Err(e) => Err(NumberParseError::from(e)),
            }
        } else {
            match s.parse::<i32>() {
                Ok(v) => {
                    if v == i32::MAX || v == i32::MIN {
                        s.parse::<i64>()
                            .map(Number::I64)
                            .map_err(NumberParseError::from)
                    } else {
                        Ok(Number::I32(v))
                    }
                }
                Err(_) => s
                    .parse::<i64>()
                    .map(Number::I64)
                    .map_err(NumberParseError::from),
            }
        }
    }
}

impl Typed for Number {
    fn get_type_info(&self) -> TypeInfo {
        TypeInfo {
            name: "Number".to_string(),
            methods: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Null {}

impl Typed for Null {
    fn get_type_info(&self) -> TypeInfo {
        TypeInfo {
            name: "Null".to_string(),
            methods: vec![],
        }
    }
}

impl Serialize for Literal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Literal::String(str) => str.serialize(serializer),
            Literal::Number(num) => num.serialize(serializer),
            Literal::Bool(bool) => bool.serialize(serializer),
            Literal::Null(null) => null.serialize(serializer),
        }
    }
}

impl Serialize for Null {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_none()
    }
}

impl TryFrom<Callee> for Literal {
    type Error = ();

    fn try_from(value: Callee) -> Result<Self, Self::Error> {
        if let Callee::Identifier(val) = value {
            return Literal::try_from(val);
        }
        Err(())
    }
}

impl ToString for Literal {
    fn to_string(&self) -> String {
        match self {
            Literal::String(str) => str.clone(),
            _ => self.to_string(),
        }
    }
}
