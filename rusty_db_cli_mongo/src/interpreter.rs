use crate::{
    lexer::{Lexer, LexerError, Token},
    parser::{ParseError, Parser},
    types::expressions::Program,
};

pub struct Interpreter {
    pub tokens: Vec<Token>,
    pub lexer_error: Option<LexerError>,
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
        Self {
            tokens: vec![],
            lexer_error: None,
        }
    }

    pub fn tokenize(mut self, data: String) -> Self {
        match Lexer::new(data).scan_tokens() {
            Ok(ok) => {
                self.tokens = ok;
            }
            Err((tokens, mut err)) => {
                self.tokens = tokens;
                self.lexer_error = Some(err.remove(0));
            }
        }

        self
    }

    pub fn parse(self) -> Result<Program, ParseError> {
        Parser::new(self.tokens).parse()
    }

    pub fn try_parse(&self) -> (Program, Option<ParseError>) {
        Parser::new(self.tokens.clone()).try_parse()
    }
}
