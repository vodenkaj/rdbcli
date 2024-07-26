use std::{
    collections::HashMap,
    fs::{create_dir, File},
    io::{Read, Write},
    path::Path,
};

use lsp_server::{Connection, ExtractError, Notification, Request, RequestId, Response};
use lsp_types::{
    notification::{DidChangeTextDocument, DidOpenTextDocument},
    request::Completion,
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, Diagnostic,
    DiagnosticServerCapabilities, DiagnosticSeverity, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, InlayHintServerCapabilities, Position, PublishDiagnosticsParams,
    Range, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
};
use rusty_db_cli_mongo::{
    interpreter::Interpreter, parser::ParseError, standard_library::StandardLibrary,
    types::expressions::Node,
};

fn main() {
    let (connection, _) = Connection::stdio();

    let server_capabilities = serde_json::to_value(ServerCapabilities {
        completion_provider: Some(lsp_types::CompletionOptions::default()),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        diagnostic_provider: Some(DiagnosticServerCapabilities::RegistrationOptions(
            lsp_types::DiagnosticRegistrationOptions::default(),
        )),
        inlay_hint_provider: Some(lsp_types::OneOf::Right(
            InlayHintServerCapabilities::RegistrationOptions(
                lsp_types::InlayHintRegistrationOptions::default(),
            ),
        )),
        ..ServerCapabilities::default()
    })
    .unwrap();

    let path = Path::new(get_config_path().as_str()).join(".collections.txt");

    if !path.exists() {
        File::create(path.clone()).expect("Failed to create collections file");
    }

    let mut file = File::open(path).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    let collections = content
        .split('\n')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    let _ = connection.initialize(server_capabilities).unwrap();

    let mut handler = Handler {
        collections,
        cache: Cache::default(),
        lib: StandardLibrary::new(),
    };

    dbg!("Initialized");

    for message in &connection.receiver {
        match message {
            lsp_server::Message::Request(req) => {
                if connection.handle_shutdown(&req).unwrap() {
                    break;
                }

                if let Ok((id, params)) = cast::<Completion>(req) {
                    if let Some(completion) = handler.handle_completion((params, id)) {
                        connection
                            .sender
                            .try_send(lsp_server::Message::Response(completion))
                            .unwrap();
                    }
                }
            }
            lsp_server::Message::Notification(notif) => {
                if let Some(notification) = handler.handle_notification(notif) {
                    connection
                        .sender
                        .try_send(lsp_server::Message::Notification(notification))
                        .unwrap();
                }
            }
            _ => (),
        };
    }
}

#[derive(Default)]
struct Cache {
    files: HashMap<String, String>,
}

struct Handler {
    collections: Vec<String>,
    cache: Cache,
    lib: StandardLibrary,
}

impl Handler {
    fn handle_completion(&self, (params, id): (CompletionParams, RequestId)) -> Option<Response> {
        let _character = params.text_document_position.position.character;
        let file_uri = params.text_document_position.text_document.uri.to_string();

        let mut debug_file = File::create("/home/janv/debug-compe.log").unwrap();

        if !self.cache.files.contains_key(&file_uri) {
            return None;
        }

        let content = self.cache.files.get(&file_uri).unwrap();
        let (program, _) = Interpreter::new().tokenize(content.clone()).try_parse();

        let tree = program.get_tree();
        let raw_type = tree.children.first().unwrap().name.clone();
        let type_info = self.lib.get_type_info(&raw_type);

        let mut items: Vec<CompletionItem> = vec![];

        if let Some(type_info) = type_info.clone() {
            let method = type_info.methods[0].clone();
            items.push(CompletionItem {
                label: method.signature,
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(method.documentation),
                ..CompletionItem::default()
            })
        }

        debug_file
            .write_all(
                format!(
                    "[raw_type]: {:?}, [type_info]: {:?}, [types]: {:?}, [test]: {:?}",
                    raw_type, type_info, self.lib.types, "db"
                )
                .as_bytes(),
            )
            .unwrap();

        //let items: Vec<CompletionItem> = self
        //    .collections
        //    .clone()
        //    .into_iter()
        //    .map(|coll| CompletionItem {
        //        label: coll.to_string(),
        //        kind: Some(CompletionItemKind::VARIABLE),
        //        detail: Some("Collection".to_owned()),
        //        ..CompletionItem::default()
        //    })
        //    .collect();

        Some(lsp_server::Response {
            id,
            result: serde_json::to_value(CompletionResponse::Array(items)).ok(),
            error: None, //error: if let Some(err) = err {
                         //    Some(ResponseError {
                         //        code: ErrorCode::ParseError as i32,
                         //        message: err.message,
                         //        data: None,
                         //    })
                         //} else {
                         //    None
                         //},
        })
    }

    fn handle_notification(&mut self, notif: Notification) -> Option<Notification> {
        dbg!("Handling notification");
        if let Ok(data) = cast_notification::<DidChangeTextDocument>(notif.clone()) {
            self.cache.on_change(data)
        } else if let Ok(data) = cast_notification::<DidOpenTextDocument>(notif) {
            self.cache.on_open(data)
        } else {
            None
        }
    }
}

impl Cache {
    pub fn on_change(&mut self, params: DidChangeTextDocumentParams) -> Option<Notification> {
        dbg!("On change");
        let file_uri = params.text_document.uri.to_string();
        if !self.files.contains_key(&file_uri) {
            dbg!("Server does not track this file - skip");
            return None;
            //return Some(Response {
            //    id: RequestId
            //    result: None,
            //    error: Some(ResponseError {
            //        code: ErrorCode::ContentModified as i32,
            //        message: "Server is not tracking this file".to_string(),
            //        data: None,
            //    }),
            //});
        }
        let file = self.files.get_mut(&file_uri).unwrap();

        for change in params.content_changes.iter() {
            *file = change.text.clone();
        }

        let content = self.files.get(&file_uri).unwrap();

        dbg!("About to tokenize");
        let error;
        let interpreter = Interpreter::new().tokenize(content.clone());

        if let Some(err) = interpreter.lexer_error {
            error = Some(ParseError {
                token_pos: err.position,
                message: err.message,
                r#type: err.token_error,
            })
        } else {
            let (_, err) = interpreter.try_parse();
            error = err;
        }

        let mut debug_file = File::create("/home/janv/debug.log").unwrap();

        if let Some(err) = error {
            let token = &interpreter.tokens.get(err.token_pos).unwrap();
            debug_file
                .write_all(
                    format!(
                        "[error]: {:?}, \n[tokens]: {:?},  \n[token]: {:?}",
                        err, interpreter.tokens, token
                    )
                    .as_bytes(),
                )
                .unwrap();
            Some(lsp_server::Notification {
                method: "textDocument/publishDiagnostics".to_string(),
                params: serde_json::to_value(PublishDiagnosticsParams {
                    uri: params.text_document.uri,
                    diagnostics: vec![Diagnostic {
                        severity: Some(DiagnosticSeverity::ERROR),
                        range: Range {
                            start: Position::new(token.line as u32, token.range.start as u32),
                            end: Position::new(token.line as u32, token.range.end as u32),
                        },
                        message: err.message,
                        ..Default::default()
                    }],
                    version: None,
                })
                .ok()
                .into(),
            })
        } else {
            debug_file
                .write_all("does not have error".as_bytes())
                .unwrap();
            Some(lsp_server::Notification {
                method: "textDocument/publishDiagnostics".to_string(),
                params: serde_json::to_value(PublishDiagnosticsParams {
                    uri: params.text_document.uri,
                    diagnostics: vec![],
                    version: None,
                })
                .ok()
                .into(),
            })
        }
    }

    pub fn on_open(&mut self, params: DidOpenTextDocumentParams) -> Option<Notification> {
        self.files.insert(
            params.text_document.uri.to_string(),
            params.text_document.text,
        );
        dbg!("Done");

        None
    }
}

fn cast_notification<N>(notif: Notification) -> Result<N::Params, ExtractError<Notification>>
where
    N: lsp_types::notification::Notification,
    N::Params: serde::de::DeserializeOwned,
{
    notif.extract(N::METHOD)
}

fn cast<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}

fn get_config_path() -> String {
    let home = home::home_dir().expect("HomeDir to be available");

    let xdg_dir = home.join(".config");
    if !xdg_dir.exists() {
        create_dir(xdg_dir.clone()).expect("Failed to create .config dir");
    }
    let xdg_dir_config = xdg_dir.join("rusty_db_cli");
    if !xdg_dir_config.exists() {
        create_dir(xdg_dir_config.clone()).expect("Failed to create .config/rusty_db_cli dir");
    }

    return xdg_dir_config.to_str().unwrap().to_string();
}
