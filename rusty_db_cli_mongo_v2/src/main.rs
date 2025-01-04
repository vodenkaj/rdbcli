use std::{cell::RefCell, fs::File, io::Read};

use rusty_db_cli_mongo_v2::parser::{get_located_span, hash};

fn main() {
    let mut content = String::new();
    File::open("query.mongo")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();

    let errors = RefCell::new(Vec::new());
    let input = get_located_span(&content, &errors);
    let res = hash(input);
    dbg!(res);
    //let res = expect(char(')'), "expected ')'")(input);
    // dbg!(parse_mongo_content(input));
    dbg!(errors);
}
