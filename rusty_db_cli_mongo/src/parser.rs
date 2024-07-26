use std::usize;

use rusty_db_cli_derive_internals::TryFrom;

use crate::{
    lexer::{Token, TokenType},
    types::{
        errors::UnexpectedTokenError,
        expressions::{
            ArrayExpression, CallExpression, CallExpressionPrimary, Callee, ExpressionStatement,
            Identifier, MemberExpression, MemberExpressionPrimary, ObjectExpression,
            ParametersExpression, Program, Property, RegexExpression,
        },
    },
};

/// Identifier              -> Literal | ObjectExpression | ArrayExpression
/// Literal                 -> String | Number | Bool | Null
/// ObjectExpression        -> "{" (Property ("," Property)*)? "}"
/// Property                -> Identifier ":" Identifier
///
/// Expressions
/// MemberExpressionPrimary -> ( Identifier | CallExpression ) "." Identifier
/// MemberExpression'       -> ((CallExpression' | Identifier) MemberExpression')?
/// CallExpression          -> (MemberExpression | Identifier) ParametersExpression CallExpression'
/// CallExpression'         -> (ParametersExpression CallExpression')?
/// ParametersExpression    -> "(" Identifier ("," Identifier)* ")"
/// ExpressionStatement     -> CallExpression
/// ArrayExpression         -> "[" (Identifier ("," Identifier)?)+ "]"

#[derive(TryFrom, Debug)]
pub enum Expression {
    Program(Program),
    ExpressionStatement(ExpressionStatement),
    Identifier(Identifier),
    CallExpression(CallExpression),
    MemberExpression(MemberExpression),
    Property(Property),
    ParametersExpression(ParametersExpression),
}

impl Expression {
    pub fn extract<T>(self) -> Result<T, String>
    where
        T: TryFrom<Self>,
    {
        let message = format!(
            "Cannot convert expression {:?} to {:?}",
            &self,
            std::any::type_name::<T>()
        );
        if let Ok(value) = match self {
            Expression::Program(val) => T::try_from(Expression::Program(val)),
            Expression::ExpressionStatement(val) => {
                T::try_from(Expression::ExpressionStatement(val))
            }
            Expression::Identifier(val) => T::try_from(Expression::Identifier(val)),
            Expression::CallExpression(val) => T::try_from(Expression::CallExpression(val)),
            Expression::MemberExpression(val) => T::try_from(Expression::MemberExpression(val)),
            Expression::Property(val) => T::try_from(Expression::Property(val)),
            Expression::ParametersExpression(val) => {
                T::try_from(Expression::ParametersExpression(val))
            }
        } {
            Ok(value)
        } else {
            Err(message)
        }
    }
}

pub struct Parser {
    pub tokens: Vec<Token>,
    pub output: Vec<Expression>,
    current: usize,
}

#[derive(Debug)]
pub struct ParseError {
    pub token_pos: usize,
    pub message: String,
    pub r#type: UnexpectedTokenError,
}

#[derive(Default)]
pub struct ParserOptions {
    pub end_after_n_exp_statements: Option<usize>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            output: Vec::new(),
            current: 0,
        }
    }

    pub fn try_parse(mut self) -> (Program, Option<ParseError>) {
        let mut last_error = None;
        loop {
            match Parser::new(self.tokens.clone()).parse() {
                Ok(ok) => {
                    return (ok, last_error);
                }
                Err(e) => {
                    let token_pos = if e.token_pos > 1 {
                        e.token_pos - 1
                    } else {
                        e.token_pos
                    };
                    if last_error.is_none() {
                        last_error = Some(e);
                    }
                    if self.tokens.is_empty() {
                        return (Program { body: Vec::new() }, last_error);
                    }
                    let (first, _) = self.tokens.split_at(token_pos);
                    self.tokens = first.to_vec();
                }
            }
        }
    }

    pub fn parse(mut self) -> Result<Program, ParseError> {
        while !self.is_at_end() {
            let expr: Result<Expression, ParseError> = match self.peek()?.r#type {
                TokenType::Identifier => {
                    if self.ensure_next_token().is_ok()
                        && (self.check_next(TokenType::Dot)?
                            || self.check_next(TokenType::LeftParen)?)
                    {
                        Ok(Expression::ExpressionStatement(
                            self.expression_statement()?,
                        ))
                    } else {
                        Ok(Expression::Identifier(self.identifier_expression()?))
                    }
                }
                _ => Err(ParseError {
                    token_pos: self.current,
                    message: format!("Expected identifier, got {:?}", self.peek()),
                    r#type: UnexpectedTokenError {
                        expected: TokenType::Identifier,
                        found: self.peek()?.r#type.clone(),
                    },
                }),
            };
            self.output.push(expr?);
        }

        Ok(Program { body: self.output })
    }

    fn expression_statement(&mut self) -> Result<ExpressionStatement, ParseError> {
        if self.check_next(TokenType::Dot)? {
            let member_expression = self.member_expression()?;
            let call_expression = self.call_expression(Callee::Member(member_expression))?;
            return Ok(ExpressionStatement {
                expression: call_expression,
            });
        }

        let identifier = self.identifier_expression()?;
        Ok(ExpressionStatement {
            expression: self.call_expression(Callee::Identifier(identifier))?,
        })
    }

    fn literal_expression(&mut self) -> Result<Identifier, ParseError> {
        match &self.peek()?.literal {
            Some(_) => Ok(Identifier::Literal(self.advance()?.literal.unwrap())),
            None => Err(ParseError {
                token_pos: self.current,
                message: format!("Expected literal, got {:?}", self.peek()),
                r#type: UnexpectedTokenError {
                    // Not entirely correct
                    expected: TokenType::Identifier,
                    found: self.peek()?.r#type.clone(),
                },
            }),
        }
    }

    fn array_expression(&mut self) -> Result<ArrayExpression, ParseError> {
        self.consume(TokenType::LeftBracket)?;

        let mut args = Vec::new();
        while !self.check(TokenType::RightBracket)? {
            let identifier = self.identifier_expression()?;

            if self.check(TokenType::LeftParen)? {
                args.push(Identifier::Call(Box::new(
                    self.call_expression(Callee::Identifier(identifier))?,
                )));
            } else {
                args.push(identifier);
            }

            if !self.check(TokenType::RightBracket)? {
                self.consume(TokenType::Comma)?;
            }
        }

        if self.is_at_end() {
            return Err(ParseError {
                token_pos: self.current.saturating_sub(1),
                message: "Expected end of array expression".to_string(),
                r#type: UnexpectedTokenError {
                    expected: TokenType::RightBracket,
                    found: TokenType::Eof,
                },
            });
        }
        self.consume(TokenType::RightBracket)?;

        Ok(ArrayExpression { elements: args })
    }

    fn regex_expression(&mut self) -> Result<Identifier, ParseError> {
        let regex = self.advance()?.literal.unwrap().to_string();
        let flags = self.advance()?.literal.unwrap().to_string();

        Ok(Identifier::Regex(RegexExpression { regex, flags }))
    }

    fn identifier_expression(&mut self) -> Result<Identifier, ParseError> {
        let value = match self.peek()?.r#type {
            TokenType::Identifier
            | TokenType::Number
            | TokenType::String
            | TokenType::Bool
            | TokenType::Null => self.literal_expression().ok(),
            TokenType::LeftBrace => Some(Identifier::Object(self.object_expression()?)),
            TokenType::LeftBracket => Some(Identifier::Array(self.array_expression()?)),
            TokenType::Regex => Some(self.regex_expression()?),
            _ => None,
        };

        match value {
            Some(val) => Ok(val),
            None => Err(ParseError {
                token_pos: self.current,
                message: format!(
                    "Expected identifier expression, got {:?} instead",
                    self.peek(),
                ),
                r#type: UnexpectedTokenError {
                    expected: TokenType::Identifier,
                    found: self.peek()?.r#type.clone(),
                },
            }),
        }
    }

    fn property_expression(&mut self) -> Result<Property, ParseError> {
        let key = self.literal_expression()?;
        self.consume(TokenType::Colon)?;
        let value = self.identifier_expression()?;

        if self.check(TokenType::LeftParen)? {
            return Ok(Property {
                key,
                value: Identifier::Call(Box::new(self.call_expression(Callee::Identifier(value))?)),
            });
        }

        Ok(Property { key, value })
    }

    fn object_expression(&mut self) -> Result<ObjectExpression, ParseError> {
        let mut props = Vec::new();
        let mut brackets = 1;
        self.advance()?;
        while brackets > 0 || self.is_at_end() {
            if self.check(TokenType::LeftBrace)? {
                brackets += 1;
                self.advance()?;
                continue;
            } else if self.check(TokenType::RightBrace)? {
                brackets -= 1;
                self.advance()?;
                continue;
            }

            props.push(self.property_expression()?);

            if self.check(TokenType::Comma)? {
                self.advance()?;
            }
        }

        if self.is_at_end() && brackets != 0 {
            return Err(ParseError {
                token_pos: self.current.saturating_sub(1),
                message: "Unexpected end of object expression".to_string(),
                r#type: UnexpectedTokenError {
                    expected: TokenType::RightBrace,
                    found: TokenType::Eof,
                },
            });
        };

        Ok(ObjectExpression { properties: props })
    }

    fn parameters_expression(&mut self) -> Result<ParametersExpression, ParseError> {
        self.consume(TokenType::LeftParen)?;

        if self.is_at_end() {
            return Err(ParseError {
                token_pos: self.current.saturating_sub(1),
                message: "Expected ')'".to_string(),
                r#type: UnexpectedTokenError {
                    expected: TokenType::RightParen,
                    found: TokenType::Eof,
                },
            });
        }

        let mut args = Vec::new();
        while !self.check(TokenType::RightParen)? {
            args.push(self.identifier_expression()?);
            if self.check(TokenType::Comma)? {
                self.advance()?;
            }
        }

        if self.is_at_end() {
            return Err(ParseError {
                token_pos: self.current.saturating_sub(1),
                message: "Unexpected end of parameters expression".to_string(),
                r#type: UnexpectedTokenError {
                    expected: TokenType::RightParen,
                    found: TokenType::Eof,
                },
            });
        }
        self.advance()?;

        Ok(ParametersExpression { params: args })
    }

    fn call_expression(&mut self, callee: Callee) -> Result<CallExpression, ParseError> {
        let primary = CallExpressionPrimary {
            params: self.parameters_expression()?,
            callee,
        };
        let recursive = self.call_expression_recursive(CallExpression::Primary(primary))?;

        Ok(recursive)
    }

    fn call_expression_recursive(
        &mut self,
        base: CallExpression,
    ) -> Result<CallExpression, ParseError> {
        if self.is_at_end() {
            return Ok(base);
        }

        if self.check(TokenType::LeftParen)? {
            let params = self.parameters_expression()?;
            return self
                .call_expression_recursive(CallExpression::Recursive(Box::new(base), params));
        }
        if self.check(TokenType::Dot)? {
            let member =
                self.member_expression_recursive(MemberExpression::Call(Box::new(base)))?;
            return self.call_expression_recursive(CallExpression::Member(Box::new(member)));
        }

        Ok(base)
    }

    fn member_expression_primary(&mut self) -> Result<MemberExpressionPrimary, ParseError> {
        let object = self.literal_expression()?;
        self.consume(TokenType::Dot)?;
        let property = self.literal_expression()?;
        Ok(MemberExpressionPrimary { object, property })
    }

    fn member_expression_recursive(
        &mut self,
        base: MemberExpression,
    ) -> Result<MemberExpression, ParseError> {
        if !self.is_at_end() && self.check(TokenType::Dot)? {
            self.consume(TokenType::Dot)?;
            let object = self.literal_expression()?;
            return self
                .member_expression_recursive(MemberExpression::Recursive(Box::new(base), object));
        }

        Ok(base)
    }

    fn member_expression(&mut self) -> Result<MemberExpression, ParseError> {
        let primary_member = self.member_expression_primary()?;

        let member = self.member_expression_recursive(MemberExpression::Primary(primary_member))?;

        Ok(member)
    }

    fn consume(&mut self, token_type: TokenType) -> Result<Token, ParseError> {
        let token = self.advance()?;

        match token_type == token.r#type {
            true => Ok(token),
            false => Err(ParseError {
                token_pos: self.current - 1,
                message: format!("Expected {:?}, got {:?}", token_type, token),
                r#type: UnexpectedTokenError {
                    expected: token_type,
                    found: token.r#type,
                },
            }),
        }
    }

    fn check(&self, token_type: TokenType) -> Result<bool, ParseError> {
        Ok(self.peek()?.r#type == token_type)
    }

    fn check_next(&self, token_type: TokenType) -> Result<bool, ParseError> {
        Ok(self.peek_next()?.r#type == token_type)
    }

    fn peek(&self) -> Result<&Token, ParseError> {
        self.ensure_token()?;
        return Ok(self.tokens.get(self.current).unwrap());
    }

    fn peek_next(&self) -> Result<&Token, ParseError> {
        self.ensure_next_token()?;
        return Ok(self.tokens.get(self.current + 1).unwrap());
    }

    fn ensure_next_token(&self) -> Result<(), ParseError> {
        if self.current + 1 >= self.tokens.len() {
            return Err(ParseError {
                token_pos: self.current.saturating_sub(1),
                message: "Unexpected end of program".to_string(),
                r#type: UnexpectedTokenError {
                    expected: TokenType::Unknown,
                    found: TokenType::Eof,
                },
            });
        }
        Ok(())
    }

    fn ensure_token(&self) -> Result<(), ParseError> {
        if self.is_at_end() {
            return Err(ParseError {
                token_pos: self.current.saturating_sub(1),
                message: "Unexpected end of program".to_string(),
                r#type: UnexpectedTokenError {
                    expected: TokenType::Unknown,
                    found: TokenType::Eof,
                },
            });
        }
        Ok(())
    }

    fn advance(&mut self) -> Result<Token, ParseError> {
        self.ensure_token()?;
        self.current += 1;
        Ok(self.tokens.get(self.current - 1).unwrap().clone())
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len()
    }
}
