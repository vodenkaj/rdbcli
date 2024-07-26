use std::collections::HashMap;

use mongodb::bson::Document;
use rusty_db_cli_mongo::{
    interpreter::{Interpreter, InterpreterError},
    parser::Expression,
    types::{
        expressions::{CallExpression, Callee, Identifier, MemberExpression, ParametersExpression},
        literals::Literal,
    },
};
use tokio_stream::StreamExt;

use super::connector::{MongodbConnector, SubCommand};
use crate::{
    connectors::{
        base::{DatabaseData, DatabaseValue, Object, PaginationInfo},
        mongodb::connector::{Command, QueryBuilder},
    },
    utils::external_editor::DEBUG_FILE,
};

pub struct InterpreterMongo<'a> {
    connector: &'a MongodbConnector,
    expressions: Vec<Expression>,
    pagination: PaginationInfo,
}

#[macro_export]
macro_rules! try_from {
    // Match against a type and a value
    (<$type:ty>($value:expr)) => {{
        match <$type>::try_from($value) {
            Ok(val) => Ok(val),
            Err(_) => Err(InterpreterError {
                message: format!("Failed to convert value to type {}", stringify!($type),),
            }),
        }
    }};
}

const MAXIMUM_DOCUMENTS: usize = 100;

impl<'a> InterpreterMongo<'a> {
    pub fn new(connector: &'a MongodbConnector, pagination: PaginationInfo) -> Self {
        Self {
            connector,
            expressions: vec![],
            pagination,
        }
    }

    pub async fn interpret(mut self, data: String) -> Result<DatabaseData, InterpreterError> {
        let mut program = Interpreter::new().tokenize(data).parse()?;
        // Our parser performs reverse-ordered tokenization and parsing,
        // -> it constructs an output array where tokens are stored in reverse order
        // compared to their original sequence in the input. And we want to execute the
        // first line first, so we reverse the array.
        program.body.reverse();

        if let Some(expression) = program.body.pop() {
            return match expression {
                Expression::ExpressionStatement(expression_statement) => {
                    return self
                        .execute_call_expression(expression_statement.expression)
                        .await;
                }
                _ => {
                    // Program should not ever have another Program in it
                    Err(InterpreterError {
                        message: "Program should only have one expression".to_string(),
                    })
                }
            };
        }

        Err(InterpreterError {
            message: "Failed to interpret data".to_string(),
        })
    }

    async fn execute_db_call(&mut self) -> Result<DatabaseData, InterpreterError> {
        if self.try_get_next_literal::<String>()? == "db" {
            let db = self.connector.get_handle();

            let collection_name = self.try_get_next_literal::<String>()?;
            let command_type = self.try_get_next_literal::<String>()?;
            let params = self.consume::<ParametersExpression>()?;
            DEBUG_FILE.write_log(&params);
            let mut main_command = Command::try_from((command_type, params))?;

            while !self.expressions.is_empty() {
                let command = self.try_get_next_literal::<String>()?;
                let params = self.consume::<ParametersExpression>()?;

                main_command.add_sub_query(SubCommand::try_from((command, params))?)?;
            }

            let collection: mongodb::Collection<Document> = db.collection(&collection_name);

            let database_response = main_command
                .build(collection, self.pagination)
                .await
                .unwrap();

            let mut result: DatabaseData = DatabaseData(Vec::new());
            match database_response {
                super::connector::DatabaseResponse::Cursor(mut cursor) => {
                    while let Some(doc) = cursor.try_next().await.unwrap() {
                        let converted_doc = try_from!(<DatabaseValue>(doc))?;
                        match converted_doc {
                            DatabaseValue::Object(obj) => {
                                result.push(obj);
                            }
                            _ => {
                                return Err(InterpreterError {
                                    message: "Database returned unexpected value".to_string(),
                                })
                            }
                        }
                        if result.len() >= MAXIMUM_DOCUMENTS {
                            break;
                        }
                    }
                }
                super::connector::DatabaseResponse::Bson(bson_arr) => {
                    for bson in bson_arr {
                        let converted_bson = try_from!(<DatabaseValue>(bson))?;
                        match converted_bson {
                            DatabaseValue::Object(obj) => {
                                result.push(obj);
                            }
                            _ => result.push(Object(HashMap::from([(
                                "result".to_string(),
                                converted_bson,
                            )]))),
                        }
                    }
                }
            }

            return Ok(result);
        };
        Err(InterpreterError {
            message: "Failed to execute db call".to_string(),
        })
    }

    fn try_get_next_literal<T: TryFrom<Literal>>(&mut self) -> Result<T, InterpreterError> {
        try_from!(<T>(try_from!(<Literal>(self.consume::<Identifier>()?))?))
    }

    fn consume<T: TryFrom<Expression>>(&mut self) -> Result<T, InterpreterError> {
        let result = self.expressions.pop().unwrap().extract::<T>();
        if let Ok(expression) = result {
            return Ok(expression);
        }

        Err(InterpreterError {
            message: format!("Failed to consume expression: {:?}", result.err()),
        })
    }

    async fn execute_call_expression(
        &mut self,
        call: CallExpression,
    ) -> Result<DatabaseData, InterpreterError> {
        self.resolve_call_expression(call);

        if self.expressions.is_empty() {
            return Err(InterpreterError {
                message: "Empty call expression".to_string(),
            });
        }
        self.execute_db_call().await
    }

    fn resolve_call_expression(&mut self, call: CallExpression) {
        match call {
            CallExpression::Primary(primary) => {
                self.expressions
                    .push(Expression::ParametersExpression(primary.params));
                match primary.callee {
                    Callee::Identifier(identifier) => {
                        self.expressions.push(Expression::Identifier(identifier))
                    }
                    Callee::Member(member) => self.resolve_member_expression(member),
                };
            }
            CallExpression::Recursive(call, params) => {
                self.expressions
                    .push(Expression::ParametersExpression(params));
                self.resolve_call_expression(*call);
            }
            CallExpression::Member(member) => self.resolve_member_expression(*member),
        };
    }

    fn resolve_member_expression(&mut self, member: MemberExpression) {
        match member {
            MemberExpression::Primary(primary) => {
                self.expressions.append(&mut vec![
                    Expression::Identifier(primary.property),
                    Expression::Identifier(primary.object),
                ]);
            }
            MemberExpression::Recursive(member, identifier) => {
                self.expressions.push(Expression::Identifier(identifier));
                self.resolve_member_expression(*member);
            }
            MemberExpression::Call(call) => self.resolve_call_expression(*call),
        }
    }
}
