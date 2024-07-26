use crate::lexer::TokenType;

#[derive(Debug)]
pub struct UnexpectedTokenError {
    pub expected: TokenType,
    pub found: TokenType,
}

#[derive(Debug)]
pub enum ErrorType {
    UnexpectedToken(UnexpectedTokenError),
}
