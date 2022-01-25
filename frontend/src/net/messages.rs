use crate::net::connection::Connection;
use crate::net::message::{Message, MessageTrait};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
pub struct LoginMessage {
    pub user_name: String,
}

impl MessageTrait for LoginMessage {
    fn process(self, _connection: &mut Connection) {}

    fn number(&self) -> u32 {
        1
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PublishGlobalChatMessage {
    pub message: String,
}

impl MessageTrait for PublishGlobalChatMessage {
    fn process(self, _connection: &mut Connection) {}

    fn number(&self) -> u32 {
        2
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GlobalChatMessage {
    pub user_name: String,
    pub message: String,
}

impl MessageTrait for GlobalChatMessage {
    fn process(self, connection: &mut Connection) {
        let mut console_messages_guard = connection.console_messages.lock().unwrap();
        console_messages_guard.push(format!("{}: {}", self.user_name, self.message));
        drop(console_messages_guard);
    }

    fn number(&self) -> u32 {
        3
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PublishPrivateChatMessage {
    pub to_user_name: String,
    pub message: String,
}

impl MessageTrait for PublishPrivateChatMessage {
    fn process(self, _connection: &mut Connection) {}

    fn number(&self) -> u32 {
        4
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PrivateChatMessage {
    pub from_user_name: String,
    pub message: String,
}

impl MessageTrait for PrivateChatMessage {
    fn process(self, connection: &mut Connection) {
        let mut console_messages_guard = connection.console_messages.lock().unwrap();
        console_messages_guard.push(format!(
            "[PRIVATE] {}: {}",
            self.from_user_name, self.message
        ));
        drop(console_messages_guard);
    }

    fn number(&self) -> u32 {
        5
    }
}
