use crate::systems::event_system::{Event, EventHandler, EventPool, EventValue};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use pam_client::{conv_mock::Conversation, Context, Flag};
use std::sync::{Arc, Mutex};

use super::window_manager::WindowCommand;

pub enum AuthCommand {
    Login(String),
}

pub struct AuthManager {
    password: Arc<Mutex<Option<String>>>,
    authenticated_at: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl AuthManager {
    pub fn new() -> Self {
        let password = Arc::new(Mutex::new(Some(String::new())));
        let authenticated_at: Arc<Mutex<Option<DateTime<Utc>>>> = Arc::new(Mutex::new(None));

        Self {
            password,
            authenticated_at,
        }
    }

    pub fn get_password(&self) -> Option<String> {
        self.password.lock().unwrap().clone()
    }

    fn authenticate(&self, password: &String) -> String {
        let mut ctx = Context::new(
            "su",
            None,
            Conversation::with_credentials(whoami::username(), password),
        )
        .expect("Failed to init pam context");
        ctx.authenticate(Flag::NONE).expect("Auth failed");
        ctx.acct_mgmt(Flag::NONE)
            .expect("Account validation failed");
        password.clone()
    }

    fn set_password(&mut self, password: String) -> String {
        *self.authenticated_at.lock().unwrap() = Some(Utc::now());
        *self.password.lock().unwrap() = Some(password.clone());
        password
    }
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventHandler for AuthManager {
    async fn on_event(&mut self, (event, pool): (&Event, Arc<Mutex<EventPool>>)) {
        if let EventValue::OnAuthCommand(cmd) = &event.value {
            match cmd {
                AuthCommand::Login(value) => {
                    let verified_pass = self.authenticate(&value);
                    self.set_password(verified_pass);
                    pool.lock().unwrap().trigger(Event {
                        component_id: event.component_id,
                        // TODO: Remove this hardcoded indexing
                        value: EventValue::OnWindowCommand(WindowCommand::SetFocusedWindow(1)),
                    });
                }
            }
        }
    }
}
