use crate::net::connection::Connection;
use serde::{Deserialize, Serialize};

macro_rules! SendMessage {
    ($connection:expr, $message:expr) => {
        let message = serde_json::to_string(&$message).expect("Error while serialize message!");
        $connection.send_message(message, $message.number());
    };
}

pub trait Message {
    fn process(&self, connection: &mut Connection);
    fn number(&self) -> u32;
}

#[derive(Serialize, Deserialize)]
pub struct GlobalChatMessage {
    pub message: String,
}

impl Message for GlobalChatMessage {
    fn process(&self, connection: &mut Connection) {
        let global_chat_message = GlobalChatMessage {
            message: "Hello from Server!".to_string(),
        };
        SendMessage!(connection, global_chat_message);
    }

    fn number(&self) -> u32 {
        0
    }
}
