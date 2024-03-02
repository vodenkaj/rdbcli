use std::{
    fs::{create_dir, File},
    io::Read,
    path::Path,
};

use lsp_server::{Connection, ExtractError, Request, RequestId};
use lsp_types::{
    request::Completion, CompletionItem, CompletionItemKind, CompletionResponse, ServerCapabilities,
};

fn main() {
    let (connection, _) = Connection::stdio();

    let server_capabilities = serde_json::to_value(ServerCapabilities {
        completion_provider: Some(lsp_types::CompletionOptions::default()),
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
    let collections = content.split('\n').collect::<Vec<&str>>();

    let _ = connection.initialize(server_capabilities).unwrap();

    for message in &connection.receiver {
        if let lsp_server::Message::Request(req) = message {
            if connection.handle_shutdown(&req).unwrap() {
                break;
            }

            if let Ok((id, params)) = cast::<Completion>(req) {
                let character = params.text_document_position.position.character;

                if (3..5).contains(&character) {
                    let items: Vec<CompletionItem> = collections
                        .clone()
                        .into_iter()
                        .map(|coll| CompletionItem {
                            label: coll.to_string(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some("Collection".to_owned()),
                            ..CompletionItem::default()
                        })
                        .collect();

                    let _ = connection.sender.try_send(lsp_server::Message::Response(
                        lsp_server::Response {
                            id,
                            result: serde_json::to_value(CompletionResponse::Array(items)).ok(),
                            error: None,
                        },
                    ));
                }
            }
        }
    }
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
