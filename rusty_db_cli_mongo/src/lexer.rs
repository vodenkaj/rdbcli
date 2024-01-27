use core::fmt;
use std::fmt::Debug;

use rusty_db_cli_derive_internals::TryFrom;
use serde::{ser::Serializer, Serialize};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TokenType {
    // Single-character tokens.
    Semicolon,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Colon,

    // Literals
    Identifier,
    String,
    Number,
    Bool,
    Regex,
    RegexFlags,
    Null,

    Eof,
    Unknown,
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug)]
pub struct LexerError {
    message: String,
    position: usize,
    line: usize,
}

#[derive(Debug, Clone, TryFrom)]
pub enum Literal {
    String(String),
    Number(f32),
    Bool(bool),
    Null(Null),
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

#[derive(Debug, Clone)]
pub struct Null {}

impl Serialize for Null {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_none()
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

#[derive(Debug, Clone)]
pub struct Token {
    pub r#type: TokenType,
    lexeme: String,
    pub literal: Option<Literal>,
    line: usize,
    position: usize,
}

impl ToString for Token {
    fn to_string(&self) -> String {
        format!("{} {} {:?}", self.r#type, self.lexeme, self.literal)
    }
}

pub struct Lexer {
    source: String,
    tokens: Vec<Token>,
    start: usize,
    current: usize,
    line: usize,
    errors: Vec<LexerError>,
}

impl Lexer {
    pub fn new(source: String) -> Self {
        Self {
            source,
            tokens: Vec::new(),
            start: 0,
            current: 0,
            line: 1,
            errors: Vec::new(),
        }
    }

    pub fn scan_tokens(mut self) -> Result<Vec<Token>, Vec<LexerError>> {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token();
        }

        if self.errors.is_empty() {
            return Ok(self.tokens);
        }
        Err(self.errors)
    }

    fn scan_token(&mut self) {
        let c = self.advance();

        // Skip whitespaces & new lines
        if c == ' ' || c == '\r' || c == '\t' {
            return;
        }
        if c == '\n' {
            self.line += 1;
            return;
        }

        match c {
            ';' => self.add_token(TokenType::Semicolon, None),
            '(' => self.add_token(TokenType::LeftParen, None),
            ')' => self.add_token(TokenType::RightParen, None),
            '{' => self.add_token(TokenType::LeftBrace, None),
            '}' => self.add_token(TokenType::RightBrace, None),
            '[' => self.add_token(TokenType::LeftBracket, None),
            ']' => self.add_token(TokenType::RightBracket, None),
            '.' => self.add_token(TokenType::Dot, None),
            ',' => self.add_token(TokenType::Comma, None),
            ':' => self.add_token(TokenType::Colon, None),
            '"' => {
                self.string();
                let mut str = self.get_current_lexeme();
                str.remove(0);
                str.pop();
                self.add_token(TokenType::String, Some(Literal::String(str)))
            }
            '/' => {
                self.regex();
                let mut str = self.get_current_lexeme();
                str.remove(0);
                str.pop();
                self.add_token(TokenType::Regex, Some(Literal::String(str)));

                self.regex_flags();
                let flags = self.get_current_lexeme();
                self.add_token(TokenType::RegexFlags, Some(Literal::String(flags)))
            }
            _ => {
                if c.is_ascii_digit() || (c == '-' && self.peek().is_numeric()) {
                    self.digit();
                    self.add_token(
                        TokenType::Number,
                        Some(Literal::Number(
                            self.get_current_lexeme().parse::<f32>().unwrap(),
                        )),
                    );
                } else if c.is_ascii_alphabetic() || (c == '$' || c == '_') {
                    self.identifier();

                    match self.get_current_lexeme().as_str() {
                        "true" => self.add_token(TokenType::Bool, Some(Literal::Bool(true))),
                        "false" => self.add_token(TokenType::Bool, Some(Literal::Bool(false))),
                        "null" => self.add_token(TokenType::Null, Some(Literal::Null(Null {}))),
                        _ => self.add_token(
                            TokenType::Identifier,
                            Some(Literal::String(self.get_current_lexeme())),
                        ),
                    }
                } else {
                    self.error("Unknown character");
                }
            }
        };
    }

    fn error(&mut self, message: &str) {
        self.errors.push(LexerError {
            message: message.to_string(),
            position: self.current,
            line: self.line,
        });
    }

    fn add_token(&mut self, r#type: TokenType, literal: Option<Literal>) {
        self.tokens.push(Token {
            r#type,
            lexeme: self.get_current_lexeme(),
            literal,
            line: self.line,
            position: self.current,
        });
    }

    fn get_current_lexeme(&self) -> String {
        self.source[self.start..self.current].to_string()
    }

    fn is_espaced_char_or_espace(&mut self, c: char) -> bool {
        self.peek() == '\\' && (self.peek_next() == c || self.peek_next() == '\\')
    }

    fn string(&mut self) {
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
            }

            if self.is_espaced_char_or_espace('"') {
                self.advance();
            }

            self.advance();
        }

        if self.is_at_end() {
            self.error("Unterminated string");
            return;
        }

        self.advance();
    }

    fn regex(&mut self) {
        while self.peek() != '/' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
            }

            if self.is_espaced_char_or_espace('/') {
                self.advance();
            }

            self.advance();
        }

        if self.is_at_end() {
            self.error("Unterminated regex");
            return;
        }

        self.advance();
    }

    fn regex_flags(&mut self) {
        let valid_regex_flags = ['i', 'm', 'x', 's', 'u'];
        while valid_regex_flags.iter().any(|&x| self.peek() == x) {
            self.advance();
        }
    }

    fn is_identifier(&mut self) -> bool {
        self.peek().is_ascii_alphabetic() || self.peek() == '$' || self.peek() == '_'
    }

    fn identifier(&mut self) {
        while self.is_identifier() {
            self.advance();
        }
    }

    fn digit(&mut self) {
        while self.peek().is_numeric() {
            self.advance();
        }

        if self.peek() == '-' && self.peek_next().is_numeric() {
            self.advance();

            while self.peek().is_numeric() {
                self.advance();
            }
        }

        if self.peek() == '.' && self.peek_next().is_numeric() {
            self.advance();

            while self.peek().is_numeric() {
                self.advance();
            }
        }
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            return '\0';
        }
        return self.source.chars().nth(self.current).unwrap();
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.source.len() {
            return '\0';
        }
        return self.source.chars().nth(self.current + 1).unwrap();
    }

    fn advance(&mut self) -> char {
        self.current += 1;
        self.source.chars().nth(self.current - 1).unwrap()
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }
}
