use crate::net::connection::Connection;
use crate::net::message::{Message, MessageTrait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PingMessage {
    pub nonce: u32,
    pub reply: bool,
}

impl MessageTrait for PingMessage {
    fn process(self, connection: &mut Connection) {
        if !self.reply {
            let reply = PingMessage {
                nonce: self.nonce,
                reply: true,
            };
            connection.send_message(Message::new(reply));
        }
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
    fn process(self, _: &mut Connection) {
        println!("{}: {}", self.name, self.message);
    }

    fn number(&self) -> u32 {
        1
    }
}
