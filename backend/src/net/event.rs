use crate::net::msg::messages::GlobalChatMessage;
use mio::Waker;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

macro_rules! EventHandler {
    ($($type_name:ident: ($function_name:ident, $variable_name:ident)),+) => {

        pub struct EventHandler {
            pub waker: Option<Arc<Waker>>,
            $(pub $variable_name: Arc<Mutex<VecDeque<$type_name>>>,)+
        }

        impl EventHandler {
            pub fn new(waker: Option<Arc<Waker>>) -> Self {
                Self {
                    waker,
                    $($variable_name: Arc::new(Mutex::new(VecDeque::new())),)+
                }
            }

            $(
                pub fn $function_name(&self, $variable_name: $type_name) {
                    let mut guard = self.$variable_name.lock().unwrap();

                    guard.push_back($variable_name);

                    drop(guard);

                    match &self.waker {
                        Some(waker) => {
                            waker.wake().expect("Error while wake!");
                        }
                        None => {}
                    }
                }
            )+
        }

        impl Clone for EventHandler {
            fn clone(&self) -> Self {
                let waker = match &self.waker {
                    Some(waker) => {
                        Some(Arc::clone(&waker))
                    }
                    None => None
                };
                Self {
                    waker: waker,
                    $($variable_name: Arc::clone(&self.$variable_name),)+
                }
            }
        }

    };
}

#[derive(Clone)]
pub struct PrivateChatMessageEvent {
    pub from_user_name: String,
    pub to_user_name: String,
    pub message: String,
}

EventHandler!(
    GlobalChatMessage: (broadcast_global_chat_message, global_chat_message_queue),
    PrivateChatMessageEvent: (private_chat_message_event, private_chat_message_event_queue)
);
