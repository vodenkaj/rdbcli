use std::{fs::File, io::Read};

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct MongoParser;

fn parse_mongo_content(content: &str) {
    match MongoParser::parse(Rule::mongo, content) {
        Ok(res) => {
            dbg!(res);
        }
        Err(err) => {
            dbg!(err);
        }
    }
}

fn parse_until_success(content: &str) {
    let mut mutable_content = content;
    loop {
        dbg!(mutable_content);
        match MongoParser::parse(Rule::mongo, mutable_content) {
            Ok(res) => {
                dbg!(res);
                break;
            }
            Err(err) => {
                dbg!(&err);
                mutable_content = get_input_without_failing_part(mutable_content, err);
                dbg!(&mutable_content);
                break;
            }
        }
    }
}

fn get_input_without_failing_part(content: &str, error: pest::error::Error<Rule>) -> &str {
    let index = match error.location {
        pest::error::InputLocation::Pos(pos) => pos,
        pest::error::InputLocation::Span(span) => {
            panic!("span location unhadled {:?}", span);
        }
    };

    let (new_content, _) = content.split_at(index.saturating_sub(2));

    new_content
}

fn main() {
    let mut content = String::new();
    File::open("query.mongo")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();

    parse_until_success(&content);
}
