use std::str::FromStr;

use bson::{oid::ObjectId, Bson, DateTime as BsonDateTime};
use chrono::{DateTime, NaiveDate, Utc};
use dyn_clone::DynClone;
use rusty_db_cli_derive_internals::{TryFrom, WithType};
use serde::{
    ser::{Error, SerializeMap},
    Serialize,
};

use super::literals::Literal;
use crate::{
    interpreter::InterpreterError,
    parser::Expression,
    standard_library::{TypeInfo, Typed},
};

#[derive(Debug, Clone, TryFrom, WithType)]
pub enum Identifier {
    Literal(Literal),
    Object(ObjectExpression),
    Array(ArrayExpression),
    Call(Box<CallExpression>),
    Regex(RegexExpression),
}

#[derive(Debug, Clone)]
pub struct RegexExpression {
    pub regex: String,
    pub flags: String,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub key: Identifier,
    pub value: Identifier,
}

#[derive(Debug)]
pub struct Program {
    pub body: Vec<Expression>,
}

#[derive(Debug, Clone)]
pub struct ObjectExpression {
    pub properties: Vec<Property>,
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

#[derive(Clone, Debug)]
pub struct MemberExpressionPrimary {
    pub object: Identifier,
    pub property: Identifier,
}

#[derive(Clone, Debug)]
pub struct CallExpressionPrimary {
    pub params: ParametersExpression,
    pub callee: Callee,
}

#[derive(Clone, Debug)]
pub struct ParametersExpression {
    pub params: Vec<Identifier>,
}

#[derive(Clone, Debug, TryFrom)]
pub enum Callee {
    Identifier(Identifier),
    Member(MemberExpression),
}

#[derive(Clone, Debug)]
pub struct ArrayExpression {
    pub elements: Vec<Identifier>,
}

impl Typed for ArrayExpression {
    fn get_type_info(&self) -> TypeInfo {
        TypeInfo {
            name: "Array".to_string(),
            methods: vec![],
        }
    }
}

impl Typed for CallExpression {
    fn get_type_info(&self) -> TypeInfo {
        TypeInfo {
            name: "Call".to_string(),
            methods: vec![],
        }
    }
}

impl Typed for RegexExpression {
    fn get_type_info(&self) -> TypeInfo {
        TypeInfo {
            name: "Regex".to_string(),
            methods: vec![],
        }
    }
}

impl Typed for ObjectExpression {
    fn get_type_info(&self) -> TypeInfo {
        TypeInfo {
            name: "Object".to_string(),
            methods: vec![],
        }
    }
}

impl Node for Callee {
    fn get_tree(&self) -> TreeNode {
        match self {
            Callee::Identifier(value) => value.get_tree(),
            Callee::Member(value) => value.get_tree(),
        }
    }
}

impl Serialize for Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Identifier::Literal(literal) => literal.serialize(serializer),
            Identifier::Object(obj) => obj.serialize(serializer),
            Identifier::Array(arr) => arr.serialize(serializer),
            Identifier::Call(call) => call.serialize(serializer),
            Identifier::Regex(regex) => bson::Regex {
                pattern: regex.regex.clone(),
                options: regex.flags.clone(),
            }
            .serialize(serializer),
        }
    }
}

impl Serialize for CallExpression {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            CallExpression::Primary(call) => {
                let key =
                    String::try_from(Literal::try_from(call.callee.clone()).unwrap()).unwrap();

                match key.as_str() {
                    "DateTime" => {
                        if call.params.params.len() > 1 {
                            return Err(Error::custom("DateTime can only have one parameter"));
                        }

                        let value =
                            String::try_from(call.params.get_nth_of_type::<Literal>(0).unwrap())
                                .unwrap();

                        match parse_date_string(&value) {
                            Ok(date) => date.serialize(serializer),
                            Err(err) => Err(Error::custom(err.message)),
                        }
                    }
                    "ObjectId" => {
                        if call.params.params.len() > 1 {
                            return Err(Error::custom("ObjectId can only have one parameter"));
                        }
                        let value =
                            String::try_from(call.params.get_nth_of_type::<Literal>(0).unwrap())
                                .unwrap();

                        ObjectId::from_str(&value).unwrap().serialize(serializer)
                    }
                    _ => Err(Error::custom("Invalid primary call expression.")),
                }
            }
            _ => Err(Error::custom(
                "Non primary call expression cannot be serialized",
            )),
        }
    }
}

impl Serialize for ParsedDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ParsedDate::Naive(naive) => {
                let dt = naive.and_hms_opt(0, 0, 0).unwrap();
                Bson::DateTime(BsonDateTime::from_chrono(
                    DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc),
                ))
                .serialize(serializer)
            }
            ParsedDate::DateTime(datetime) => {
                Bson::DateTime(BsonDateTime::from_chrono(*datetime)).serialize(serializer)
            }
        }
    }
}

enum ParsedDate {
    Naive(NaiveDate),
    DateTime(DateTime<Utc>),
}

fn parse_date_string(date_str: &str) -> Result<ParsedDate, InterpreterError> {
    // First, try to parse as NaiveDate
    if let Ok(naive) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return Ok(ParsedDate::Naive(naive));
    }

    // Next, try to parse as DateTime with a timezone
    if let Ok(datetime) = DateTime::parse_from_rfc3339(date_str) {
        return Ok(ParsedDate::DateTime(datetime.with_timezone(&Utc)));
    }

    // If both attempts fail, return an error
    Err(InterpreterError {
        message: format!("Expected valid date string, got {} instead", date_str),
    })
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

impl Serialize for ObjectExpression {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(None)?;

        for prop in self.properties.iter() {
            map.serialize_entry(&prop.key, &prop.value)?;
        }

        map.end()
    }
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

impl Node for Property {
    fn get_tree(&self) -> TreeNode {
        TreeNode {
            name: "Property".to_string(),
            children: vec![self.key.get_tree(), self.value.get_tree()],
        }
    }
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
            Identifier::Regex(_) => TreeNode {
                name: "Regex".to_string(),
                children: vec![],
            },
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

impl ParametersExpression {
    pub fn get_nth_of_type<T: TryFrom<Identifier>>(
        &self,
        nth: usize,
    ) -> Result<T, InterpreterError> {
        if nth >= self.params.len() {
            return Err(InterpreterError {
                message: format!(
                    "Expected parameter at index {} but got {} parameters",
                    nth,
                    self.params.len()
                ),
            });
        }

        match T::try_from(self.params.get(nth).unwrap().clone()) {
            Ok(value) => Ok(value),
            Err(_) => Err(InterpreterError {
                message: "Failed to convert parameter".to_string(),
            }),
        }
    }
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

impl Serialize for ArrayExpression {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.elements.serialize(serializer)
    }
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

pub trait Node {
    fn get_tree(&self) -> TreeNode;
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
