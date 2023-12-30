use crate::ui::components::connection::ConnectionInfo;

pub enum ConnectionEvent {
    Add(ConnectionInfo),
    Connect(ConnectionInfo),
    SwitchDatabase(String),
}

pub struct ConnectionManager {
    pub connections: Vec<ConnectionInfo>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
        }
    }

    pub fn add_connection(&mut self, info: ConnectionInfo) {
        self.connections.push(info);
    }
}
