use rust_chat_client::net::client::Client;
use rust_chat_client::net::message::Message;
use rust_chat_client::net::messages::{
    LoginMessage, PublishGlobalChatMessage, PublishPrivateChatMessage,
};
use std::io;

fn main() {
    println!("Enter your name: ");
    let mut name = String::new();
    io::stdin()
        .read_line(&mut name)
        .expect("Error while reading line!");
    name = name.trim().to_string();

    let address = "127.0.0.1:4444"
        .parse()
        .expect("Error while parsing address!");

    let mut client = Client::new(address);

    println!("Connecting...");
    let client_stop = client.connect();
    let login_message = LoginMessage { user_name: name };
    client.send_message(Message::new(login_message));
    println!("Connected.");

    loop {
        let mut message = String::new();
        match io::stdin().read_line(&mut message) {
            Ok(_) => {
                message = message.trim().to_string();

                if message == "exit" {
                    return;
                }

                if message.starts_with("private ") {
                    let split = message.split(" ").collect::<Vec<&str>>();

                    let private_chat_message = PublishPrivateChatMessage {
                        to_user_name: split[1].to_string(),
                        message: split[2..].join(" "),
                    };
                    client.send_message(Message::new(private_chat_message));
                } else {
                    let global_chat_message = PublishGlobalChatMessage { message };
                    client.send_message(Message::new(global_chat_message));
                }
            }
            Err(_) => break,
        }
    }

    client_stop.stop();
    client.join();
}
