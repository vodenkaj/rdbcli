use std::{fs::File, io::Read};

use rusty_db_cli_mongo::{interpreter::Interpreter, parser::Node};

fn main() {
    let mut file = File::open("query.mongo").unwrap();
    let mut buff = String::new();
    file.read_to_string(&mut buff).unwrap();

    Interpreter::new()
        .tokenize(buff)
        .unwrap()
        .parse()
        .unwrap()
        .get_tree()
        .print()
}
