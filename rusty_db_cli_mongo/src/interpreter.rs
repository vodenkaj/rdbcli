use crate::{
    lexer::{Lexer, LexerError, Token},
    parser::{ParseError, Parser, Program},
};

pub struct Interpreter {
    tokens: Vec<Token>,
}

#[derive(Debug)]
pub struct InterpreterError {
    pub message: String,
}

#[macro_export]
macro_rules! to_interpter_error {
    ($result: expr) => {
        match $result {
            Ok(value) => Ok(value),
            Err(err) => Err(InterpreterError {
                message: err.to_string(),
            }),
        }
    };
}

impl From<LexerError> for InterpreterError {
    fn from(err: LexerError) -> Self {
        Self {
            message: format!("{:?}", err),
        }
    }
}

impl From<Vec<LexerError>> for InterpreterError {
    fn from(err: Vec<LexerError>) -> Self {
        Self {
            message: format!("{:?}", err),
        }
    }
}

impl From<ParseError> for InterpreterError {
    fn from(err: ParseError) -> Self {
        Self {
            message: format!("{:?}", err),
        }
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self { tokens: vec![] }
    }

    pub fn tokenize(mut self, data: String) -> Result<Self, Vec<LexerError>> {
        let result = Lexer::new(data).scan_tokens()?;
        self.tokens = result;

        Ok(self)
    }

    pub fn parse(self) -> Result<Program, ParseError> {
        Parser::new(self.tokens).parse()
    }
}
