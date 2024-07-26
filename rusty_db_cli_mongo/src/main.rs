use std::{fs::File, io::Read};

use rusty_db_cli_mongo::{interpreter::Interpreter, types::expressions::Node};

fn main() {
    let mut file = File::open("query.mongo").unwrap();
    let mut buff = String::new();
    file.read_to_string(&mut buff).unwrap();

    Interpreter::new()
        .tokenize(buff)
        .parse()
        .unwrap()
        .get_tree()
        .print()
}
