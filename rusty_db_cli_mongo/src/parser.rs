use crate::lexer::{Literal, Token, TokenType};
use dyn_clone::DynClone;
use rusty_db_cli_derive_internals::TryFrom;
use std::{convert::From, usize};

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

impl Node for Expression {
    fn get_tree(&self) -> TreeNode {
        match self {
            Expression::Program(program) => program.get_tree(),
            Expression::ExpressionStatement(expression_statement) => {
                expression_statement.get_tree()
            }
            Expression::Identifier(identifier) => identifier.get_tree(),
            Expression::CallExpression(call) => call.get_tree(),
            Expression::MemberExpression(member) => member.get_tree(),
            Expression::Property(prop) => prop.get_tree(),
            Expression::ParametersExpression(params) => params.get_tree(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CallExpressionPrimary {
    pub params: ParametersExpression,
    pub callee: Callee,
}

#[derive(Clone, Debug)]
pub enum Callee {
    Identifier(Identifier),
    Member(MemberExpression),
}

impl Node for Callee {
    fn get_tree(&self) -> TreeNode {
        match self {
            Callee::Identifier(value) => value.get_tree(),
            Callee::Member(value) => value.get_tree(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MemberExpressionPrimary {
    pub object: Identifier,
    pub property: Identifier,
}

#[derive(Clone, Debug)]
pub enum MemberExpression {
    Primary(MemberExpressionPrimary),
    Recursive(Box<MemberExpression>, Identifier),
    Call(Box<CallExpression>),
}

#[derive(Clone, Debug)]
pub enum CallExpression {
    Primary(CallExpressionPrimary),
    Recursive(Box<CallExpression>, ParametersExpression),
    Member(Box<MemberExpression>),
}

impl Node for CallExpression {
    fn get_tree(&self) -> TreeNode {
        match self {
            CallExpression::Member(val) => val.get_tree(),
            CallExpression::Primary(val) => val.get_tree(),
            CallExpression::Recursive(value, params) => TreeNode {
                name: "CallExpression".to_owned(),
                children: vec![value.get_tree(), params.get_tree()],
            },
        }
    }
}

impl Node for MemberExpression {
    fn get_tree(&self) -> TreeNode {
        match self {
            MemberExpression::Call(value) => value.get_tree(),
            MemberExpression::Primary(value) => value.get_tree(),
            MemberExpression::Recursive(value, identifier) => TreeNode {
                name: "MemberExpression".to_owned(),
                children: vec![value.get_tree(), identifier.get_tree()],
            },
        }
    }
}

impl From<MemberExpression> for Callee {
    fn from(val: MemberExpression) -> Self {
        Callee::Member(val)
    }
}

impl From<Identifier> for Callee {
    fn from(val: Identifier) -> Self {
        Callee::Identifier(val)
    }
}

impl Expressable for MemberExpressionPrimary {
    fn get_name(&self) -> String {
        format!("{} - {:?}", stringify!(MemberExpression), self.property)
    }
}

impl Node for MemberExpressionPrimary {
    fn get_tree(&self) -> TreeNode {
        TreeNode {
            name: "MemberExpression".to_string(),
            children: vec![self.object.get_tree(), self.property.get_tree()],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectExpression {
    pub properties: Vec<Property>,
}

impl Node for ObjectExpression {
    fn get_tree(&self) -> TreeNode {
        TreeNode {
            name: "ObjectExpression".to_string(),
            children: self
                .properties
                .clone()
                .into_iter()
                .map(|p| p.get_tree())
                .collect(),
        }
    }
}

#[derive(Debug)]
pub struct Program {
    pub body: Vec<Expression>,
}

impl Node for Program {
    fn get_tree(&self) -> TreeNode {
        TreeNode {
            name: "Program".to_string(),
            children: self.body.iter().map(|x| x.get_tree()).collect(),
        }
    }
}

#[derive(Debug)]
pub struct ExpressionStatement {
    pub expression: CallExpression,
}

impl Node for ExpressionStatement {
    fn get_tree(&self) -> TreeNode {
        TreeNode {
            name: "ExpressionStatement".to_string(),
            children: vec![self.expression.get_tree()],
        }
    }
}

pub trait Node {
    fn get_tree(&self) -> TreeNode;
}

#[derive(Debug, Clone)]
pub struct Property {
    pub key: Identifier,
    pub value: Identifier,
}

impl Node for Property {
    fn get_tree(&self) -> TreeNode {
        TreeNode {
            name: "Property".to_string(),
            children: vec![self.key.get_tree(), self.value.get_tree()],
        }
    }
}

#[derive(Debug, Clone, TryFrom)]
pub enum Identifier {
    Literal(Literal),
    Object(ObjectExpression),
    Array(ArrayExpression),
    Call(Box<CallExpression>),
}

impl Node for Identifier {
    fn get_tree(&self) -> TreeNode {
        match self {
            Identifier::Literal(value) => TreeNode {
                name: format!("Identifier [{:?}]", value),
                children: vec![],
            },
            Identifier::Object(value) => value.get_tree(),
            Identifier::Array(value) => value.get_tree(),
            Identifier::Call(value) => value.get_tree(),
        }
    }
}

impl Expressable for CallExpressionPrimary {
    fn get_name(&self) -> String {
        "CallExpression".to_string()
    }
}

impl Node for CallExpressionPrimary {
    fn get_tree(&self) -> TreeNode {
        TreeNode {
            name: "CallExpression".to_string(),
            children: vec![self.callee.get_tree(), self.params.get_tree()],
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParametersExpression {
    pub params: Vec<Identifier>,
}

impl Node for ParametersExpression {
    fn get_tree(&self) -> TreeNode {
        TreeNode {
            name: "ParametersExpression".to_string(),
            children: self
                .params
                .clone()
                .into_iter()
                .map(|p| p.get_tree())
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ArrayExpression {
    pub elements: Vec<Identifier>,
}

impl Node for ArrayExpression {
    fn get_tree(&self) -> TreeNode {
        TreeNode {
            name: "ArrayExpression".to_string(),
            children: self
                .elements
                .clone()
                .into_iter()
                .map(|p| p.get_tree())
                .collect(),
        }
    }
}

trait Expressable: Node + DynClone {
    fn get_name(&self) -> String;
}

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

#[derive(Debug)]
pub struct ParseError {
    pub token_pos: usize,
    pub message: String,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse(mut self) -> Result<Program, ParseError> {
        let mut program = Program { body: Vec::new() };
        while !self.is_at_end() {
            let expr: Result<Expression, ParseError> = match self.peek()?.r#type {
                TokenType::Identifier => {
                    if self.check_next(TokenType::Dot)? || self.check_next(TokenType::LeftParen)? {
                        Ok(Expression::ExpressionStatement(
                            self.expression_statement()?,
                        ))
                    } else {
                        Err(ParseError {
                            token_pos: self.current,
                            message: "Expected expression, got identifier".to_string(),
                        })
                    }
                }
                _ => Err(ParseError {
                    token_pos: self.current,
                    message: format!("Expected identifier, got {:?}", self.peek()),
                }),
            };
            program.body.push(expr?);
        }

        Ok(program)
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
            }),
        }
    }

    fn array_expression(&mut self) -> Result<ArrayExpression, ParseError> {
        self.consume(TokenType::LeftBracket)?;

        let mut args = Vec::new();
        while !self.check(TokenType::RightBracket)? {
            args.push(self.identifier_expression()?);

            if !self.check(TokenType::RightBracket)? {
                self.consume(TokenType::Comma)?;
            }
        }

        if self.is_at_end() {
            return Err(ParseError {
                token_pos: self.current,
                message: "Expected end of array expression".to_string(),
            });
        }
        self.consume(TokenType::RightBracket)?;

        Ok(ArrayExpression { elements: args })
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
            _ => None,
        };

        match value {
            Some(val) => Ok(val),
            None => Err(ParseError {
                token_pos: self.current,
                message: format!("Expected primary expression, got {:?} instead", self.peek(),),
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
                token_pos: self.current,
                message: "Unexpected end of object expression".to_string(),
            });
        };

        Ok(ObjectExpression { properties: props })
    }

    fn parameters_expression(&mut self) -> Result<ParametersExpression, ParseError> {
        self.consume(TokenType::LeftParen)?;
        let mut args = Vec::new();
        while !self.check(TokenType::RightParen)? {
            args.push(self.identifier_expression()?);
            if self.check(TokenType::Comma)? {
                self.advance()?;
            }
        }

        if self.is_at_end() {
            return Err(ParseError {
                token_pos: self.current,
                message: "Unexpected end of parameters expression".to_string(),
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
        if self.is_at_end() {
            return Err(ParseError {
                token_pos: self.current,
                message: "Unexpected end of member expression".to_string(),
            });
        }

        if self.check(TokenType::Dot)? {
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
        if self.is_at_end() {
            return Err(ParseError {
                token_pos: self.current,
                message: "Unexpected end of program".to_string(),
            });
        }
        Ok(())
    }

    fn ensure_token(&self) -> Result<(), ParseError> {
        if self.current + 1 > self.tokens.len() {
            return Err(ParseError {
                token_pos: self.current,
                message: "Unexpected end of program".to_string(),
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

#[derive(Default)]
pub struct PrintOptions {
    offset: usize,
    next_on_same_level: bool,
    edges: Vec<bool>,
}

impl TreeNode {
    pub fn print(&self) {
        self.recursive_print(PrintOptions::default());
    }

    fn recursive_print(
        &self,
        PrintOptions {
            offset,
            next_on_same_level,
            mut edges,
        }: PrintOptions,
    ) {
        let modified_offset = if offset == 0 { offset } else { offset + 2 };
        let pipe = if next_on_same_level {
            edges.push(true);
            "├─"
        } else {
            edges.push(false);
            "└─"
        };

        let bar: String = (0..modified_offset)
            .map(|i| {
                if i % 3 == 0 && edges[i / 3] {
                    return "│";
                }
                " "
            })
            .collect();

        println!("{}{} ({})", bar, pipe, self.name);

        for (idx, child) in self.children.iter().enumerate() {
            child.recursive_print(PrintOptions {
                offset: modified_offset + 1,
                next_on_same_level: idx != self.children.len() - 1,
                edges: edges.clone(),
            });
        }
    }
}

pub struct TreeNode {
    pub name: String,
    pub children: Vec<TreeNode>,
}
