use crate::net::connection::Connection;
use crate::net::event::PrivateChatMessageEvent;
use crate::net::msg::message::{Message, MessageTrait};
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
    fn process(self, connection: &mut Connection) {
        connection.user_name = Some(self.user_name);
    }

    fn number(&self) -> u32 {
        1
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PublishGlobalChatMessage {
    pub message: String,
}

impl MessageTrait for PublishGlobalChatMessage {
    fn process(self, connection: &mut Connection) {
        match &connection.user_name {
            Some(user_name) => {
                let reply = GlobalChatMessage {
                    user_name: user_name.clone(),
                    message: self.message,
                };
                connection
                    .server_event_handler
                    .broadcast_global_chat_message(reply);
            }
            None => {}
        }
    }

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
    fn process(self, _connection: &mut Connection) {}

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
    fn process(self, connection: &mut Connection) {
        match &connection.user_name {
            Some(user_name) => {
                let reply = PrivateChatMessageEvent {
                    from_user_name: user_name.clone(),
                    to_user_name: self.to_user_name,
                    message: self.message,
                };
                connection
                    .server_event_handler
                    .private_chat_message_event(reply);
            }
            None => {}
        }
    }

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
    fn process(self, _connection: &mut Connection) {}

    fn number(&self) -> u32 {
        5
    }
}
