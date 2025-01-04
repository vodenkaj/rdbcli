use std::{cell::RefCell, collections::HashMap, ops::Range, usize};

use nom::{
    branch::alt,
    bytes::complete::{escaped, tag, take_while},
    character::complete::{alpha1, alphanumeric0, char, one_of},
    combinator::{cut, map, value},
    error::{context, ContextError, ParseError},
    multi::{many1, separated_list0},
    sequence::{preceded, separated_pair, terminated},
    Parser,
};

pub fn get_located_span<'a>(content: &'a str, errors: &'a RefCell<Vec<Error>>) -> LocatedSpan<'a> {
    LocatedSpan::new_extra(&content, State(errors))
}

#[derive(Debug)]
enum Kind {
    CallExpression,
    Literal(String),
    Object(Vec<(Box<ParsedValue>, Box<ParsedValue>)>),
    KeyValue(Box<ParsedValue>, Box<ParsedValue>),
}

#[derive(Debug)]
pub struct ParsedValue {
    kind: Kind,
    range: Range<usize>,
}

enum Arguments {
    String,
}

type IResult<'a, T> = nom::IResult<LocatedSpan<'a>, T>;

//pub fn find_parent_node(error: &Error, node: &ParsedValue) {
//    match &node.kind {
//        Kind::Object(entries) => {
//        }
//        _ => node,
//    }
//}

fn expect<'a, F, E, T>(mut parser: F, error_msg: E) -> impl FnMut(LocatedSpan<'a>) -> IResult<T>
where
    F: FnMut(LocatedSpan<'a>) -> IResult<T>,
    E: ToString,
{
    move |input| match parser(input) {
        Ok((remaining, out)) => Ok((remaining, out)),
        Err(e) => {
            let input = match e {
                nom::Err::Incomplete(needed) => {
                    todo!()
                }
                nom::Err::Error(e) => e.input,
                nom::Err::Failure(e) => e.input,
            };
            let err = Error(input.to_range(), error_msg.to_string());
            input.extra.report_error(err); // Push error onto stack.
            Err(nom::Err::Error(nom::error::Error {
                input,
                code: nom::error::ErrorKind::Fail,
            })) // Parsing failed, but keep going.
        }
    }
}

fn expect_soft<'a, F, E, T>(
    mut parser: F,
    error_msg: E,
) -> impl FnMut(LocatedSpan<'a>) -> IResult<Option<T>>
where
    F: FnMut(LocatedSpan<'a>) -> IResult<T>,
    E: ToString,
{
    move |input| match parser(input) {
        Ok((remaining, out)) => Ok((remaining, Some(out))),
        Err(e) => {
            let input = match e {
                nom::Err::Incomplete(needed) => {
                    todo!()
                }
                nom::Err::Error(e) => e.input,
                nom::Err::Failure(e) => e.input,
            };
            let err = Error(input.to_range(), error_msg.to_string());
            input.extra.report_error(err); // Push error onto stack.
            Ok((input, None)) // Parsing failed, but keep going.
        }
    }
}

/// This used in place of `&str` or `&[u8]` in our `nom` parsers.
type LocatedSpan<'a> = nom_locate::LocatedSpan<&'a str, State<'a>>;

trait ToRange {
    fn to_range(&self) -> Range<usize>;
}

impl<'a> ToRange for LocatedSpan<'a> {
    fn to_range(&self) -> Range<usize> {
        let start = self.location_offset();
        let end = start + self.fragment().len();
        start..end
    }
}

/// Error containing a text span and an error message to display.
#[derive(Debug)]
pub struct Error(Range<usize>, String);

/// Carried around in the `LocatedSpan::extra` field in
/// between `nom` parsers.
#[derive(Clone, Debug)]
pub struct State<'a>(pub &'a RefCell<Vec<Error>>);

impl<'a> State<'a> {
    /// Pushes an error onto the errors stack from within a `nom`
    /// parser combinator while still allowing parsing to continue.
    pub fn report_error(&self, error: Error) {
        self.0.borrow_mut().push(error);
    }
}

type PResult<'a> = IResult<'a, Option<ParsedValue>>;

fn literal_base(i: LocatedSpan) -> IResult<'_, ParsedValue> {
    map(many1(alt((alpha1, tag("_")))), |spans: Vec<LocatedSpan>| {
        let mut start = usize::MAX;
        let mut end = 0;
        let mut value = String::new();
        for span in spans {
            value += span.fragment();
            let range = span.to_range();
            start = start.min(range.start);
            end = end.max(range.end);
        }

        ParsedValue {
            kind: Kind::Literal(value),
            range: Range { start, end },
        }
    })(i)
}

fn literal_soft(i: LocatedSpan) -> IResult<'_, Option<ParsedValue>> {
    expect_soft(literal_base, "expected literal")(i)
}

fn literal(i: LocatedSpan) -> IResult<ParsedValue> {
    expect(literal_base, "expected literal")(i)
}

/// SYNTAX
/// literal           = (a-Z0-9_)+
/// string            = ".*" | '.*'
/// number            = (+|-)?0-9(.0-9)?
/// bool              = true | false
/// regex             = /.*/.*
/// value             = string | number | bool | regex
/// key_value         = string | literal : value
/// object            = { key_value (, key_value)* } | { }
/// array             = [ value (, value)* ] | [ ]
/// member_expression = literal . (literal | call_expression)
/// call_expression   = literal()

fn hash_base(i: LocatedSpan) -> IResult<'_, ParsedValue> {
    map(
        preceded(
            char('{'),
            cut(terminated(
                separated_list0(char(','), key_value_error),
                preceded(space, char('}')),
            )),
        ),
        |values| {
            let mut start = usize::MAX;
            let mut end = 0;
            let values = values
                .into_iter()
                .filter_map(|val| {
                    if let Some(val) = val {
                        start = start.min(val.range.start);
                        end = end.max(val.range.end);
                        if let Kind::KeyValue(key, value) = val.kind {
                            return Some((key, value));
                        }
                    }

                    None
                })
                .collect();

            let kind = Kind::Object(values);
            ParsedValue {
                kind,
                range: Range { start, end },
            }
        },
    )(i)
}
pub fn hash_soft(i: LocatedSpan) -> PResult {
    expect_soft(hash_base, "expected object")(i)
}

pub fn hash(i: LocatedSpan) -> IResult<ParsedValue> {
    expect(hash_base, "expected object")(i)
}

fn key_value_base(i: LocatedSpan) -> IResult<'_, ParsedValue> {
    map(
        separated_pair(
            literal_base,
            preceded(char(':'), space),
            alt((hash_base, literal_base)),
        ),
        |(key, value)| {
            let f_value = value;
            let f_key = key;
            let start = f_key.range.start;
            let end = f_value.range.end;

            let kind = Kind::KeyValue(Box::new(f_key), Box::new(f_value));

            ParsedValue {
                kind,
                range: Range { start, end },
            }
        },
    )(i)
}

fn key_value_error(i: LocatedSpan) -> PResult {
    expect_soft(key_value_base, "expected key value")(i)
}

#[derive(Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Str(String),
    Boolean(bool),
    Num(f64),
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

/// parser combinators are constructed from the bottom up:
/// first we write parsers for the smallest elements (here a space character),
/// then we'll combine them in larger parsers
fn sp<'a, E: ParseError<&'a str>>(i: &'a str) -> nom::IResult<&'a str, &'a str, E> {
    let chars = " \t\r\n";

    // nom combinators like `take_while` return a function. That function is the
    // parser,to which we can pass the input
    take_while(move |c| chars.contains(c))(i)
}

fn space<'a>(i: LocatedSpan) -> IResult<'_, LocatedSpan> {
    let chars = " \t\r\n";

    // nom combinators like `take_while` return a function. That function is the
    // parser,to which we can pass the input
    take_while(move |c| chars.contains(c))(i)
}

/// A nom parser has the following signature:
/// `Input -> IResult<Input, Output, Error>`, with `IResult` defined as:
/// `type IResult<I, O, E = (I, ErrorKind)> = Result<(I, O), Err<E>>;`
///
/// most of the times you can ignore the error type and use the default (but this
/// examples shows custom error types later on!)
///
/// Here we use `&str` as input type, but nom parsers can be generic over
/// the input type, and work directly with `&[u8]` or any other type that
/// implements the required traits.
///
/// Finally, we can see here that the input and output type are both `&str`
/// with the same lifetime tag. This means that the produced value is a subslice
/// of the input data. and there is no allocation needed. This is the main idea
/// behind nom's performance.
fn parse_str<'a, E: ParseError<&'a str>>(i: &'a str) -> nom::IResult<&'a str, &'a str, E> {
    escaped(alphanumeric0, '\\', one_of("\"n\\"))(i)
}

/// `tag(string)` generates a parser that recognizes the argument string.
///
/// we can combine it with other functions, like `value` that takes another
/// parser, and if that parser returns without an error, returns a given
/// constant value.
///
/// `alt` is another combinator that tries multiple parsers one by one, until
/// one of them succeeds
fn boolean<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, bool, E> {
    // This is a parser that returns `true` if it sees the string "true", and
    // an error otherwise
    let parse_true = value(true, tag("true"));

    // This is a parser that returns `false` if it sees the string "false", and
    // an error otherwise
    let parse_false = value(false, tag("false"));

    // `alt` combines the two parsers. It returns the result of the first
    // successful parser, or an error
    alt((parse_true, parse_false)).parse(input)
}

fn null<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, (), E> {
    value((), tag("null")).parse(input)
}

/// this parser combines the previous `parse_str` parser, that recognizes the
/// interior of a string, with a parse to recognize the double quote character,
/// before the string (using `preceded`) and after the string (using `terminated`).
///
/// `context` and `cut` are related to error management:
/// - `cut` transforms an `Err::Error(e)` in `Err::Failure(e)`, signaling to
/// combinators like  `alt` that they should not try other parsers. We were in the
/// right branch (since we found the `"` character) but encountered an error when
/// parsing the string
/// - `context` lets you add a static string to provide more information in the
/// error chain (to indicate which parser had an error)
fn string<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
    i: &'a str,
) -> nom::IResult<&'a str, &'a str, E> {
    context(
        "string",
        preceded(char('\"'), cut(terminated(parse_str, char('\"')))),
    )
    .parse(i)
}

// some combinators, like `separated_list0` or `many0`, will call a parser repeatedly,
// accumulating results in a `Vec`, until it encounters an error.
// If you want more control on the parser application, check out the `iterator`
// combinator (cf `examples/iterator.rs`)
//fn array<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
//    i: &'a str,
//) -> nom::IResult<&'a str, Vec<JsonValue>, E> {
//    context(
//        "array",
//        preceded(
//            char('['),
//            cut(terminated(
//                separated_list0(preceded(sp, char(',')), json_value),
//                preceded(sp, char(']')),
//            )),
//        ),
//    )
//    .parse(i)
//}

//fn key_value<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
//    i: &'a str,
//) -> nom::IResult<&'a str, (&'a str, JsonValue), E> {
//    separated_pair(
//        preceded(sp, string),
//        cut(preceded(sp, char(':'))),
//        json_value,
//    )
//    .parse(i)
//}

//fn hash<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
//    i: &'a str,
//) -> nom::IResult<&'a str, HashMap<String, JsonValue>, E> {
//    context(
//        "map",
//        preceded(
//            char('{'),
//            cut(terminated(
//                map(
//                    separated_list0(preceded(sp, char(',')), key_value),
//                    |tuple_vec| {
//                        tuple_vec
//                            .into_iter()
//                            .map(|(k, v)| (String::from(k), v))
//                            .collect()
//                    },
//                ),
//                preceded(sp, char('}')),
//            )),
//        ),
//    )
//    .parse(i)
//}

// here, we apply the space parser before trying to parse a value
//fn json_value<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
//    i: &'a str,
//) -> nom::IResult<&'a str, JsonValue, E> {
//    preceded(
//        sp,
//        alt((
//            map(hash, JsonValue::Object),
//            map(array, JsonValue::Array),
//            map(string, |s| JsonValue::Str(String::from(s))),
//            map(double, JsonValue::Num),
//            map(boolean, JsonValue::Boolean),
//            map(null, |_| JsonValue::Null),
//        )),
//    )
//    .parse(i)
//}
//
///// the root element of a JSON parser is either an object or an array
//fn root<'a, E: ParseError<&'a str> + ContextError<&'a str>>(
//    i: &'a str,
//) -> nom::IResult<&'a str, JsonValue, E> {
//    delimited(
//        sp,
//        alt((
//            map(hash, JsonValue::Object),
//            map(array, JsonValue::Array),
//            map(null, |_| JsonValue::Null),
//        )),
//        opt(sp),
//    )
//    .parse(i)
//}
