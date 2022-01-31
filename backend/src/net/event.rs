use crate::net::msg::messages::GlobalChatMessage;
use mio::Waker;
use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc,
};

macro_rules! EventHandler {
    ($($type_name:ident: ($function_name:ident, $variable_name_sender:ident, $variable_name_receiver:ident,)),+) => {

        pub struct EventHandler {
            pub waker: Arc<Waker>,
            $(pub $variable_name_sender: Sender<$type_name>,)+
        }

        impl EventHandler {
            pub fn new(waker: Arc<Waker>) -> (Self $(,Receiver<$type_name>)+) {
                $(let ($variable_name_sender, $variable_name_receiver) = channel::<$type_name>();)+
                (Self {
                    waker,
                    $($variable_name_sender,)+
                } $(,$variable_name_receiver)+)
            }

            $(
                pub fn $function_name(&self, $variable_name_sender: $type_name) {
                    self.$variable_name_sender
                        .send($variable_name_sender)
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
                    $($variable_name_sender: self.$variable_name_sender.clone(),)+
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
    GlobalChatMessage:
        (
            broadcast_global_chat_message,
            global_chat_message_sender,
            global_chat_message_receiver,
        ),
    PrivateChatMessageEvent:
        (
            private_chat_message_event,
            private_chat_message_event_sender,
            private_chat_message_event_receiver,
        )
);
