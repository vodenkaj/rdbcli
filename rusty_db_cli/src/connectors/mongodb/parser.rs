//use super::connector::{
//    MainCommand, SubCommand, DATE_TO_STRING_REGEX, KEY_TO_STRING_REGEX, OBJECT_ID_TO_STRING_REGEX,
//    REGEX_TO_STRING_REGEX,
//};
//use anyhow::anyhow;
//use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
//use mongodb::bson::{self, oid::ObjectId, Bson, Document};
//use regex::Regex;
//use std::time::SystemTime;
//
//#[derive(PartialEq, Debug)]
//struct ProcessedResult<T> {
//    value: Option<T>,
//    rest: Option<String>,
//}
//
//struct ValueBetweenCharsOptions {
//    value: String,
//    start: char,
//    end: char,
//    include_chars: bool,
//}
//
//fn get_value_between_chars(opts: ValueBetweenCharsOptions) -> ProcessedResult<String> {
//    let mut inside_str = false;
//    let mut command: Option<String> = None;
//    let mut brackets = Vec::new();
//    let mut chars_processed = 0;
//    let chars: Vec<char> = opts.value.chars().collect();
//    for (idx, ch) in chars.iter().cloned().enumerate() {
//        chars_processed += 1;
//        if !brackets.is_empty() {
//            command = Some(command.map_or_else(|| ch.to_string(), |s| s + &ch.to_string()));
//        }
//        let is_escaped = if idx > 0 {
//            chars[idx - 1] == '\\'
//        } else {
//            false
//        };
//        match ch {
//            opening_char if opts.start == opening_char => {
//                if !inside_str && !is_escaped {
//                    brackets.push(ch);
//                }
//            }
//            closing_char if opts.end == closing_char => {
//                if !inside_str && !is_escaped {
//                    brackets.pop();
//                    if brackets.is_empty() {
//                        if let Some(cmd) = command.as_mut() {
//                            cmd.pop();
//                        }
//                        break;
//                    }
//                }
//            }
//            '"' | '\'' => {
//                if !is_escaped {
//                    inside_str = !inside_str;
//                }
//            }
//            _ => {}
//        }
//    }
//
//    if !brackets.is_empty() {
//        command = None;
//    }
//
//    let final_value = command.map(|cmd| {
//        if opts.include_chars {
//            format!("{}{}{}", opts.start, cmd, opts.end)
//        } else {
//            cmd
//        }
//    });
//    let rest: String = chars.split_at(chars_processed).1.iter().collect();
//
//    ProcessedResult {
//        value: final_value,
//        rest: if rest.trim().is_empty() {
//            None
//        } else {
//            Some(rest)
//        },
//    }
//}
//
//pub enum ParsedValue {
//    Query(CommandQuery),
//}
//
//pub struct CommandQuery {
//    pub collection_name: String,
//    pub command: CommandQueryPair<MainCommand>,
//    pub sub_commands: Vec<CommandQueryPair<SubCommand>>,
//}
//
//pub struct CommandQueryPair<T> {
//    pub command_type: T,
//    pub query: Vec<Document>,
//}
//
//pub fn parse_mongo_query(value: &str) -> anyhow::Result<ParsedValue> {
//    let trimmed_value = value.trim();
//    if trimmed_value.starts_with('\\') {
//        // Command
//        todo!()
//    } else {
//        // Query
//        let expression = clean_value(trimmed_value);
//        let ProcessedResult { value, rest } = get_value_until_char(ValueUntilCharParams {
//            value: expression.to_string(),
//            until: '.',
//            ending_char_in_rest: false,
//        });
//        if value.is_none() || value.clone().unwrap().to_lowercase() != "db" {
//            return Err(anyhow!(
//                "Expected start of query to contain 'db', found '{}' instead.",
//                value.unwrap_or("None".to_string())
//            ));
//        };
//
//        let ProcessedResult {
//            value: collection_name,
//            rest,
//        } = get_value_until_char(ValueUntilCharParams {
//            value: rest.unwrap_or_default(),
//            until: '.',
//            ending_char_in_rest: false,
//        });
//        if collection_name.is_none() {
//            return Err(anyhow!("Collection name cannot be empty"));
//        }
//
//        let ProcessedResult {
//            value: command_name,
//            rest,
//        } = get_value_until_char(ValueUntilCharParams {
//            value: rest.unwrap_or_default(),
//            until: '(',
//            ending_char_in_rest: true,
//        });
//        let validated_command: MainCommand = command_name.unwrap_or_default().parse().unwrap();
//
//        let ProcessedResult { value, rest } = get_value_between_chars(ValueBetweenCharsOptions {
//            value: rest.unwrap_or_default(),
//            start: '(',
//            end: ')',
//            include_chars: false,
//        });
//        let main_command = match value {
//            Some(value) => match validated_command {
//                MainCommand::Find => extract_find_query(value),
//                MainCommand::Aggregate => extract_aggregation_query(value),
//                MainCommand::Count => extract_count_query(value),
//            },
//            None => anyhow::Ok(Vec::new()),
//        }?
//        .into_iter()
//        .map(|cmd| validate_query(Some(&cmd)))
//        .collect::<anyhow::Result<Vec<_>>>()?;
//
//        let mut sub_commands = Vec::new();
//        let mut rest = rest;
//        while let Some(value) = rest {
//            if value.starts_with('.') {
//                let ProcessedResult {
//                    value,
//                    rest: new_rest,
//                } = get_value_until_char(ValueUntilCharParams {
//                    value: value.split_at(1).1.to_string(),
//                    until: '(',
//                    ending_char_in_rest: true,
//                });
//                let command_type = SubCommand::from_str(&value.unwrap_or_default())?;
//
//                let ProcessedResult {
//                    value,
//                    rest: new_rest,
//                } = get_value_between_chars(ValueBetweenCharsOptions {
//                    value: new_rest.unwrap(),
//                    start: '(',
//                    end: ')',
//                    include_chars: false,
//                });
//
//                sub_commands.push(CommandQueryPair {
//                    command_type,
//                    query: vec![validate_query(value.as_ref())?],
//                });
//                rest = new_rest;
//            } else {
//                rest = None;
//            }
//        }
//
//        Ok(ParsedValue::Query(CommandQuery {
//            collection_name: collection_name.unwrap(),
//            command: CommandQueryPair {
//                command_type: validated_command,
//                query: main_command,
//            },
//            sub_commands,
//        }))
//    }
//}
//
//fn clean_value(value: &str) -> String {
//    let mut is_inside_str = false;
//    let mut is_espaced = false;
//    let mut is_in_regex = false;
//    let mut cleaned_value = String::new();
//    for ch in value.chars() {
//        if (ch == ' ' || ch == '\n') && !is_in_regex && !is_espaced && !is_inside_str {
//            continue;
//        }
//
//        if ch == '/' && !is_inside_str && !is_espaced {
//            is_in_regex = !is_in_regex;
//        }
//
//        if ch == '\"' && !is_espaced && !is_in_regex {
//            is_inside_str = !is_inside_str;
//        }
//
//        if ch != '\\' {
//            is_espaced = false;
//        } else if !is_inside_str && !is_in_regex {
//            is_espaced = !is_espaced;
//        }
//
//        cleaned_value += &ch.to_string();
//    }
//
//    cleaned_value
//}
//fn extract_count_query(value: String) -> anyhow::Result<Vec<String>> {
//    let count_query = get_value_between_chars(ValueBetweenCharsOptions {
//        value,
//        start: '{',
//        end: '}',
//        include_chars: true,
//    });
//
//    if let Some(rest) = count_query.rest {
//        return Err(anyhow!(
//            "Invalid count query parameters.
//                            Expected end of string, found {} instead.",
//            rest
//        ));
//    }
//
//    anyhow::Ok(vec![count_query.value.unwrap_or_default()])
//}
//
//fn extract_aggregation_query(value: String) -> anyhow::Result<Vec<String>> {
//    let ProcessedResult { value, rest } = get_value_between_chars(ValueBetweenCharsOptions {
//        value,
//        start: '[',
//        end: ']',
//        include_chars: false,
//    });
//
//    if let Some(rest) = rest {
//        return Err(anyhow!(
//            "Invalid aggregation query parameters.
//                            Expected end of string, found {} instead.",
//            rest
//        ));
//    }
//
//    if value.is_none() {
//        return anyhow::Ok(Vec::new());
//    }
//
//    let mut pipelines = Vec::new();
//    let mut next_pipeline = value;
//    while let Some(pipeline) = next_pipeline {
//        let ProcessedResult { value, rest } = get_value_between_chars(ValueBetweenCharsOptions {
//            value: pipeline,
//            start: '{',
//            end: '}',
//            include_chars: true,
//        });
//
//        if let Some(value) = value {
//            pipelines.push(value);
//        } else if let Some(rest) = rest {
//            return Err(anyhow!(
//                "Invalid aggregation query parameters.
//                            Expected end of string, found {} instead.",
//                rest
//            ));
//        }
//        next_pipeline = rest;
//    }
//
//    anyhow::Ok(pipelines)
//}
//
//fn extract_find_query(value: String) -> anyhow::Result<Vec<String>> {
//    let ProcessedResult { value, rest } = get_value_between_chars(ValueBetweenCharsOptions {
//        value,
//        start: '{',
//        end: '}',
//        include_chars: true,
//    });
//
//    if rest.is_none() {
//        return anyhow::Ok(vec![value.unwrap_or_default()]);
//    }
//
//    let project_query = get_value_between_chars(ValueBetweenCharsOptions {
//        value: rest.clone().unwrap(),
//        start: '{',
//        end: '}',
//        include_chars: true,
//    });
//
//    if project_query.value.is_none() {
//        return Err(anyhow!(
//            "Invalid projection. Expected valid projection, found '{}' instead.",
//            rest.unwrap_or_default()
//        ));
//    }
//    if let Some(rest) = project_query.rest {
//        return Err(anyhow!(
//            "Invalid projection. Expected end of string, found '{}' instead.",
//            rest
//        ));
//    }
//
//    anyhow::Ok(vec![
//        value.unwrap_or_default(),
//        project_query.value.unwrap_or_default(),
//    ])
//}
//
//fn validate_query(query: Option<&String>) -> anyhow::Result<Document> {
//    if let Some(query) = query {
//        if query.is_empty() {
//            return Ok(Document::new());
//        }
//        let mut str_fixed = Regex::new(KEY_TO_STRING_REGEX)?
//            .replace_all(query, "\"$1\":")
//            .to_string();
//        str_fixed = Regex::new(REGEX_TO_STRING_REGEX)?
//            .replace_all(&str_fixed, "\"/$1/\"")
//            .to_string();
//        str_fixed = Regex::new(DATE_TO_STRING_REGEX)?
//            .replace_all(&str_fixed, "\"$1\"")
//            .to_string();
//        str_fixed = Regex::new(OBJECT_ID_TO_STRING_REGEX)?
//            .replace_all(&str_fixed, "\"$1\"")
//            .to_string();
//        let json_object: serde_json::Map<String, serde_json::Value> =
//            serde_json::from_str(&str_fixed)?;
//        let mut doc = Document::try_from(json_object)?;
//        doc.iter_mut().for_each(|(_, value)| resolve(value));
//        return Ok(doc);
//    }
//    Err(anyhow!("Invalid query"))
//}
//
//fn resolve(value: &mut Bson) {
//    match value {
//        Bson::String(str) => {
//            if let Some(result) = Regex::new(REGEX_TO_STRING_REGEX).unwrap().captures(str) {
//                *value = mongodb::bson::Bson::RegularExpression(bson::Regex {
//                    pattern: result.get(1).unwrap().as_str().to_string(),
//                    options: String::new(),
//                });
//            } else if let Some(result) = Regex::new(DATE_TO_STRING_REGEX).unwrap().captures(str) {
//                let raw_date = result.get(2).unwrap().as_str().to_string();
//
//                let date_time = match NaiveDate::parse_from_str(&raw_date, "%Y-%m-%d") {
//                    Ok(parsed_date) => {
//                        // Create a NaiveDateTime at midnight for the given date
//                        NaiveDateTime::new(
//                            parsed_date,
//                            NaiveTime::from_num_seconds_from_midnight_opt(0, 0).unwrap(),
//                        )
//                    }
//                    Err(e) => {
//                        panic!("Failed to parse date: {}", e);
//                    }
//                };
//
//                let date = DateTime::from_timestamp(date_time.timestamp(), 0).unwrap();
//                *value =
//                    mongodb::bson::Bson::DateTime(bson::DateTime::from(SystemTime::from(date)));
//            } else if let Some(result) =
//                Regex::new(OBJECT_ID_TO_STRING_REGEX).unwrap().captures(str)
//            {
//                let raw_object_id = result.get(2).unwrap().as_str().to_string();
//                *value = mongodb::bson::Bson::ObjectId(ObjectId::parse_str(raw_object_id).unwrap())
//            }
//        }
//        Bson::Document(doc) => doc.iter_mut().for_each(|(_, v)| resolve(v)),
//        _ => {}
//    }
//}
//
//struct ValueUntilCharParams {
//    value: String,
//    until: char,
//    ending_char_in_rest: bool,
//}
//fn get_value_until_char(params: ValueUntilCharParams) -> ProcessedResult<String> {
//    let mut is_escaped = false;
//    let mut extracted_value = String::new();
//    for ch in params.value.chars() {
//        if ch == params.until {
//            if ch == '\\' && is_escaped {
//                extracted_value += &ch.to_string();
//                break;
//            } else if ch != '\\' && !is_escaped {
//                break;
//            }
//        }
//        extracted_value += &ch.to_string();
//        is_escaped = ch == '\\';
//    }
//
//    let rest = if params.value.len() > extracted_value.len() {
//        let mut at = extracted_value.len();
//        if !params.ending_char_in_rest {
//            at += 1;
//        }
//        params.value.split_at(at).1.to_string()
//    } else {
//        String::new()
//    };
//
//    ProcessedResult {
//        value: if extracted_value.is_empty() {
//            None
//        } else {
//            Some(extracted_value)
//        },
//        rest: if rest.is_empty() { None } else { Some(rest) },
//    }
//}
//
//#[cfg(test)]
//mod tests {
//    use super::extract_find_query;
//    use crate::connectors::mongodb::parser::{
//        clean_value, extract_aggregation_query, extract_count_query, get_value_between_chars,
//        get_value_until_char, ProcessedResult, ValueBetweenCharsOptions, ValueUntilCharParams,
//    };
//
//    #[test]
//    fn get_value_until_char_with_normal_string() {
//        let value = get_value_until_char(ValueUntilCharParams {
//            value: "test.{}".to_string(),
//            until: '.',
//            ending_char_in_rest: true,
//        });
//        assert_eq!(
//            value,
//            ProcessedResult {
//                value: Some("test".to_string()),
//                rest: Some(".{}".to_string())
//            }
//        );
//    }
//
//    #[test]
//    fn get_value_until_char_with_line_breaks() {
//        let value = get_value_until_char(ValueUntilCharParams {
//            value: "test\n\nafter line break\n.dot".to_string(),
//            until: '.',
//            ending_char_in_rest: true,
//        });
//        assert_eq!(
//            value,
//            ProcessedResult {
//                value: Some("test\n\nafter line break\n".to_string()),
//                rest: Some(".dot".to_string())
//            }
//        );
//    }
//
//    #[test]
//    fn get_value_until_char_with_escapes() {
//        let value = get_value_until_char(ValueUntilCharParams {
//            value: r"before espace\. after espace.dot".to_string(),
//            until: '.',
//            ending_char_in_rest: true,
//        });
//        assert_eq!(
//            value,
//            ProcessedResult {
//                value: Some(r"before espace\. after espace".to_string()),
//                rest: Some(".dot".to_string())
//            }
//        );
//    }
//
//    #[test]
//    fn get_value_until_backslash_with_escapes() {
//        let value = get_value_until_char(ValueUntilCharParams {
//            value: r"before espace\. after espace \\ after backslash".to_string(),
//            until: '\\',
//            ending_char_in_rest: true,
//        });
//        assert_eq!(
//            value,
//            (ProcessedResult {
//                value: Some(r"before espace\. after espace \\".to_string()),
//                rest: Some(" after backslash".to_string())
//            })
//        );
//    }
//
//    #[test]
//    fn clean_value_spaces() {
//        let value = clean_value(r"({   })");
//        assert_eq!(value, r"({})");
//    }
//
//    #[test]
//    fn clean_value_with_string() {
//        let value = clean_value(r##"(   {string:"/test string\n\\"})"##);
//        assert_eq!(value, r##"({string:"/test string\n\\"})"##);
//    }
//
//    #[test]
//    fn clean_value_with_regex() {
//        let value = clean_value(r##"({regex:/test regex \n \/\n/})"##);
//        assert_eq!(value, r##"({regex:/test regex \n \/\n/})"##);
//    }
//
//    #[test]
//    fn get_value_between_parenthesis() {
//        let ProcessedResult { value, rest } = get_value_between_chars(ValueBetweenCharsOptions {
//            value: "test(this should be extracted)this should be skipped".to_string(),
//            start: '(',
//            end: ')',
//            include_chars: false,
//        });
//        assert_eq!(value, Some(String::from("this should be extracted")));
//        assert_eq!(rest, Some(String::from("this should be skipped")));
//    }
//
//    #[test]
//    fn get_value_between_parenthesis_with_nested_paranthesis() {
//        let ProcessedResult { value, rest } = get_value_between_chars(ValueBetweenCharsOptions {
//            value: r#"test((this) (should) be "()())))" (extracted))this should be skipped"#
//                .to_string(),
//            start: '(',
//            end: ')',
//            include_chars: false,
//        });
//        assert_eq!(
//            value,
//            Some(String::from(r#"(this) (should) be "()())))" (extracted)"#))
//        );
//        assert_eq!(rest, Some(String::from("this should be skipped")));
//    }
//
//    #[test]
//    fn get_value_between_parenthesis_with_escaped_paranthesis() {
//        let ProcessedResult { value, rest } = get_value_between_chars(ValueBetweenCharsOptions {
//            value: r#"te\(st(this should \)\(\)\(\( be extracted)this should be skipped"#
//                .to_string(),
//            start: '(',
//            end: ')',
//            include_chars: false,
//        });
//        assert_eq!(
//            value,
//            Some(String::from(r#"this should \)\(\)\(\( be extracted"#))
//        );
//        assert_eq!(rest, Some(String::from("this should be skipped")));
//    }
//
//    #[test]
//    fn extract_find_query_test() -> anyhow::Result<()> {
//        assert_eq!(
//            extract_find_query("{test:{$gte: 100}}, {_id: 1, title: 1}".to_string())?,
//            vec![
//                "{test:{$gte: 100}}".to_string(),
//                "{_id: 1, title: 1}".to_string()
//            ]
//        );
//        anyhow::Ok(())
//    }
//
//    #[test]
//    fn extract_aggregation_query_test() -> anyhow::Result<()> {
//        assert_eq!(
//            extract_aggregation_query(
//                "[{$match:{test:1,proj:1}}, {$project: {test: 1}}]".to_string()
//            )?,
//            vec![
//                "{$match:{test:1,proj:1}}".to_string(),
//                "{$project: {test: 1}}".to_string()
//            ]
//        );
//        anyhow::Ok(())
//    }
//
//    #[test]
//    fn extract_count_query_test() -> anyhow::Result<()> {
//        assert_eq!(
//            extract_count_query("{_id: {$gte: 100}}".to_string())?,
//            vec!["{_id: {$gte: 100}}".to_string(),]
//        );
//        anyhow::Ok(())
//    }
//}
