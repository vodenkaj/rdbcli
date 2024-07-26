use core::fmt;
use std::{fmt::Debug, str::FromStr};

use crate::types::{
    errors::UnexpectedTokenError,
    literals::{Literal, Null, Number},
};

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
    pub message: String,
    pub position: usize,
    pub line: usize,
    pub token_error: UnexpectedTokenError,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub r#type: TokenType,
    lexeme: String,
    pub literal: Option<Literal>,
    pub line: usize,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}

impl Range {
    pub fn is_value_within(&self, value: usize) -> bool {
        self.start <= value && self.end >= value
    }
}

impl ToString for Token {
    fn to_string(&self) -> String {
        format!("{} {} {:?}", self.r#type, self.lexeme, self.literal)
    }
}

pub struct Lexer {
    source: String,
    source_bytes: Vec<u8>,
    tokens: Vec<Token>,
    start: usize,
    start_relative: usize,
    end: usize,
    current: usize,
    current_relative: usize,
    current_in_bytes: usize,
    line: usize,
    current_string: String,
    errors: Vec<LexerError>,
}

impl Lexer {
    pub fn new(source: String) -> Self {
        Self {
            current_string: String::new(),
            end: source.chars().count(),
            source_bytes: source.bytes().collect(),
            source,
            tokens: Vec::new(),
            start: 0,
            start_relative: 0,
            current: 0,
            current_relative: 0,
            current_in_bytes: 0,
            line: 0,
            errors: Vec::new(),
        }
    }

    pub fn scan_tokens(mut self) -> Result<Vec<Token>, (Vec<Token>, Vec<LexerError>)> {
        while !self.is_at_end() {
            self.start = self.current;
            self.start_relative = self.current_relative;
            self.current_string = String::new();
            self.scan_token();
        }

        if self.errors.is_empty() {
            return Ok(self.tokens);
        }
        Err((self.tokens, self.errors))
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
            ';' => self.add_token(TokenType::Semicolon),
            '(' => self.add_token(TokenType::LeftParen),
            ')' => self.add_token(TokenType::RightParen),
            '{' => self.add_token(TokenType::LeftBrace),
            '}' => self.add_token(TokenType::RightBrace),
            '[' => self.add_token(TokenType::LeftBracket),
            ']' => self.add_token(TokenType::RightBracket),
            '.' => self.add_token(TokenType::Dot),
            ',' => self.add_token(TokenType::Comma),
            ':' => self.add_token(TokenType::Colon),
            '"' | '\'' => match self.string(c) {
                Ok(()) => {
                    self.add_token(TokenType::String);
                }
                Err(()) => self.add_token(TokenType::Unknown),
            },
            '/' => {
                match self.regex() {
                    Ok(_) => {
                        self.add_token(TokenType::Regex);
                        self.start = self.current;
                    }
                    Err(_) => self.add_token(TokenType::Unknown),
                }
                match self.regex_flags() {
                    Ok(_) => self.add_token(TokenType::RegexFlags),
                    Err(_) => self.add_token(TokenType::Unknown),
                }
            }
            _ => {
                if c.is_ascii_digit() || (c == '-' && self.peek().is_numeric()) {
                    match self.digit() {
                        Ok(_) => {
                            self.add_token(TokenType::Number);
                        }
                        Err(_) => {
                            self.add_token(TokenType::Unknown);
                        }
                    }
                } else if c.is_alphabetic() || (c == '$' || c == '_') {
                    match self.identifier() {
                        Ok(_) => self.add_token(TokenType::Identifier),
                        Err(_) => self.add_token(TokenType::Unknown),
                    }
                } else {
                    self.add_token(TokenType::Unknown);
                    self.error(
                        "Unknown character",
                        UnexpectedTokenError {
                            expected: TokenType::Unknown,
                            found: TokenType::Unknown,
                        },
                    );
                }
            }
        };
    }

    fn error(&mut self, message: &str, error: UnexpectedTokenError) {
        self.errors.push(LexerError {
            message: message.to_string(),
            position: self.tokens.len() - 1,
            line: self.line,
            token_error: error,
        });
    }

    fn add_token(&mut self, r#type: TokenType) {
        let lexeme = self.current_string.clone();

        let mut token_type = r#type.clone();

        let literal = match r#type {
            // We are using serde_json::from_str to parse the string,
            // because it handles new lines correctly.
            // https://d3lm.medium.com/rust-beware-of-escape-sequences-85ec90e9e243
            TokenType::String => {
                let mut data = self.current_string.chars();
                // To handle case, where string starts with "'"
                data.next();
                data.next_back();

                match serde_json::from_str(format!("\"{}\"", data.as_str()).as_str()) {
                    Ok(value) => Some(Literal::String(value)),
                    Err(_) => {
                        token_type = TokenType::Unknown;
                        None
                    }
                }
            }
            TokenType::Bool => match lexeme.as_str() {
                "true" => Some(Literal::Bool(true)),
                "false" => Some(Literal::Bool(false)),
                _ => None,
            },
            TokenType::Identifier => match lexeme.as_str() {
                "true" => Some(Literal::Bool(true)),
                "false" => Some(Literal::Bool(false)),
                "null" => Some(Literal::Null(Null {})),
                _ => Some(Literal::String(lexeme.to_string())),
            },
            TokenType::Null => Some(Literal::Null(Null {})),
            TokenType::Number => Some(Literal::Number(Number::from_str(&lexeme).unwrap())),
            TokenType::Regex => {
                let regex_value = lexeme[lexeme.chars().next().unwrap().len_utf8()
                    ..lexeme.len() - lexeme.chars().next_back().unwrap().len_utf8()]
                    .to_string();
                Some(Literal::String(regex_value))
            }
            TokenType::RegexFlags => Some(Literal::String(lexeme.to_string())),
            _ => None,
        };

        self.tokens.push(Token {
            r#type: token_type,
            literal,
            range: Range {
                start: self.start,
                end: self.current - 1,
            },
            line: self.line,
            lexeme: lexeme.to_string(),
        });
    }

    fn is_espaced_char_or_espace(&mut self, c: char) -> bool {
        self.peek() == '\\' && (self.peek_next() == c || self.peek_next() == '\\')
    }

    fn string(&mut self, str_variant: char) -> Result<(), ()> {
        while self.peek() != str_variant && !self.is_at_end() {
            if self.peek() == '\n' {
                self.current_relative = 0;
            }

            if self.is_espaced_char_or_espace(str_variant) {
                self.advance();
            }

            self.advance();
        }

        if self.is_at_end() {
            self.error(
                "Unterminated string",
                UnexpectedTokenError {
                    expected: TokenType::String,
                    found: TokenType::Eof,
                },
            );
            return Err(());
        }

        self.advance();
        Ok(())
    }

    fn regex(&mut self) -> Result<(), ()> {
        while self.peek() != '/' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.current_relative = 0;
            }

            if self.is_espaced_char_or_espace('/') {
                self.advance();
            }

            self.advance();
        }

        if self.is_at_end() {
            self.error(
                "Unterminated regex",
                UnexpectedTokenError {
                    expected: TokenType::Regex,
                    found: TokenType::Eof,
                },
            );
            return Err(());
        }

        self.advance();
        Ok(())
    }

    fn regex_flags(&mut self) -> Result<(), ()> {
        let valid_regex_flags = ['i', 'm', 'x', 's', 'u'];
        while valid_regex_flags.iter().any(|&x| self.peek() == x) {
            self.advance();
        }

        Ok(())
    }

    fn is_identifier(&mut self) -> bool {
        self.peek().is_ascii_alphabetic() || self.peek() == '$' || self.peek() == '_'
    }

    fn identifier(&mut self) -> Result<(), ()> {
        while self.is_identifier() {
            self.advance();
        }

        Ok(())
    }

    fn digit(&mut self) -> Result<(), ()> {
        while self.peek().is_numeric() {
            self.advance();
        }

        if self.peek() == '.' && self.peek_next().is_numeric() {
            self.advance();

            while self.peek().is_numeric() {
                self.advance();
            }
        }

        Ok(())
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            return '\0';
        }

        let end_index = self.current_in_bytes
            + self.utf8_char_width(*self.source_bytes.get(self.current_in_bytes).unwrap());
        let bytes = &self.source_bytes[self.current_in_bytes..end_index];

        std::str::from_utf8(bytes).unwrap().chars().next().unwrap()
    }

    fn utf8_char_width(&self, leading_byte: u8) -> usize {
        if leading_byte & 0x80 == 0 {
            1 // ASCII byte
        } else if leading_byte & 0xE0 == 0xC0 {
            2 // 110xxxxx
        } else if leading_byte & 0xF0 == 0xE0 {
            3 // 1110xxxx
        } else if leading_byte & 0xF8 == 0xF0 {
            4 // 11110xxx
        } else {
            panic!("Invalid leading byte");
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.source.len() {
            return '\0';
        }
        return self.source.chars().nth(self.current + 1).unwrap();
    }

    fn advance(&mut self) -> char {
        let ch = self.peek();
        let len = ch.len_utf8();

        self.current_in_bytes += len;
        self.current += 1;
        self.current_string += &ch.to_string();
        self.current_relative += len;

        ch
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.end
    }
}
