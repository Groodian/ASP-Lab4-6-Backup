use crate::net::msg::messages::GlobalChatMessage;
use mio::Waker;
use std::sync::{mpsc::Sender, Arc};

macro_rules! EventHandler {
    ($($type_name:ident: ($function_name:ident, $variable_name:ident)),+) => {

        pub struct EventHandler {
            pub waker: Arc<Waker>,
            $(pub $variable_name: Sender<$type_name>,)+
        }

        impl EventHandler {
            pub fn new(waker: Arc<Waker>, $($variable_name: Sender<$type_name>,)+ ) -> Self {
                Self {
                    waker,
                    $($variable_name,)+
                }
            }

            $(
                pub fn $function_name(&self, $variable_name: $type_name) {
                    self.$variable_name
                        .send($variable_name)
                        .expect(format!("Error while sending event: {}", stringify!($function_name)).as_str());
                    self.waker
                        .wake()
                        .expect(format!("Error while wake for event: {}", stringify!($function_name)).as_str());
                }
            )+
        }

        impl Clone for EventHandler {
            fn clone(&self) -> Self {
                Self {
                    waker: Arc::clone(&self.waker),
                    $($variable_name: self.$variable_name.clone(),)+
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
    GlobalChatMessage: (broadcast_global_chat_message, global_chat_message_sender),
    PrivateChatMessageEvent:
        (
            private_chat_message_event,
            private_chat_message_event_sender
        )
);
