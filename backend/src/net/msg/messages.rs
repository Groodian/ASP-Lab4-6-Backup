use crate::net::connection::Connection;
use crate::net::msg::message::{Message, MessageTrait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PingMessage {
    pub nonce: u32,
    pub reply: bool,
}

impl MessageTrait for PingMessage {
    fn process(self, connection: &mut Connection) {
        let reply = PingMessage {
            nonce: self.nonce,
            reply: true,
        };
        connection.send_message(Message::new(reply));
    }

    fn number(&self) -> u32 {
        0
    }
}

#[derive(Serialize, Deserialize)]
pub struct GlobalChatMessage {
    pub name: String,
    pub message: String,
}

impl MessageTrait for GlobalChatMessage {
    fn process(self, connection: &mut Connection) {
        let reply = GlobalChatMessage {
            name: self.name,
            message: self.message,
        };
        connection.server_broadcast_message(Message::new(reply));
    }

    fn number(&self) -> u32 {
        1
    }
}
