use crate::{
    lexer::{Lexer, LexerError, Token},
    parser::{ParseError, Parser, Program},
};

pub struct Interpreter {
    tokens: Vec<Token>,
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
