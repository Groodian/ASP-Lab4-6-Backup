use crate::net::connection::Connection;
use serde::Serialize;

pub struct Message {
    pub message: String,
    pub number: u32,
}

impl Message {
    pub fn new<T: MessageTrait + Serialize>(message: T) -> Self {
        let message_string =
            serde_json::to_string(&message).expect("Error while serialize message!");

        Self {
            message: message_string,
            number: message.number(),
        }
    }
}

impl Clone for Message {
    fn clone(&self) -> Self {
        Self {
            message: self.message.clone(),
            number: self.number.clone(),
        }
    }
}

pub trait MessageTrait {
    fn process(self, connection: &mut Connection);
    fn number(&self) -> u32;
}
