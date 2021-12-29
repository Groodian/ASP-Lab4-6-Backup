use crate::net::connection::Connection;
use serde::{Deserialize, Serialize};

pub trait Message {
    fn process(&self, connection: &mut Connection);
}

#[derive(Serialize, Deserialize)]
pub struct GlobalChatMessage {
    pub message: String,
}

impl Message for GlobalChatMessage {
    fn process(&self, connection: &mut Connection) {
        println!("GlobalChatMessage: {}", self.message);
        let test = GlobalChatMessage {
            message: "Hello from Server!".to_string(),
        };
        let test = serde_json::to_string(&test).unwrap();
        connection.send_message(test);
    }
}
