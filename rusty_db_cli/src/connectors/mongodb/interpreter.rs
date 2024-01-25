use mongodb::{bson::Document, options::FindOptions};
use rusty_db_cli_mongo::{
    interpreter::Interpreter,
    lexer::{LexerError, Literal},
    parser::{
        CallExpression, Callee, Expression, Identifier, MemberExpression, ParametersExpression,
        ParseError,
    },
};
use tokio_stream::StreamExt;

use crate::connectors::{
    base::DatabaseData,
    mongodb::connector::{Command, QueryBuilder},
};

use super::connector::MongodbConnector;

#[derive(Debug)]
pub struct InterpreterError {
    pub message: String,
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

pub struct InterpreterMongo<'a> {
    connector: &'a MongodbConnector,
    expressions: Vec<Expression>,
}

macro_rules! try_from {
    // Match against a type and a value
    (<$type:ty>($value:expr)) => {{
        match <$type>::try_from($value) {
            Ok(val) => Ok(val),
            Err(_) => Err(InterpreterError {
                message: "TryFrom failed".to_string(),
            }),
        }
    }};
}

impl<'a> InterpreterMongo<'a> {
    pub fn new(connector: &'a MongodbConnector) -> Self {
        Self {
            connector,
            expressions: vec![],
        }
    }

    pub async fn interpret(mut self, data: String) -> Result<DatabaseData, InterpreterError> {
        let mut program = Interpreter::new().tokenize(data)?.parse()?;

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
            let mut command_type: Command = self.try_get_next_literal::<String>()?.parse()?;
            let params = self.consume::<ParametersExpression>()?;

            while !self.expressions.is_empty() {
                let command = self.try_get_next_literal::<String>()?;
                let params = self.consume::<ParametersExpression>()?;

                command_type.add_sub_query(command.parse()?, params);
            }

            let collection: mongodb::Collection<Document> = db.collection(&collection_name);

            let mut cursor = command_type.build(collection).await.unwrap();
            let mut result: DatabaseData = DatabaseData(Vec::new());

            while let Some(doc) = cursor.try_next().await.unwrap() {
                result.push(serde_json::to_value(doc).unwrap());
                if result.len() >= 100 as usize {
                    break;
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

// Rules?
