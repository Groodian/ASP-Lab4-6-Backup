use rust_chat_client::net::client::Client;
use rust_chat_client::net::message::Message;
use rust_chat_client::net::messages::GlobalChatMessage;
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
    println!("Connected.");

    loop {
        let mut message = String::new();
        match io::stdin().read_line(&mut message) {
            Ok(_) => {
                message = message.trim().to_string();

                if message == "exit" {
                    return;
                }

                let global_chat_message = GlobalChatMessage {
                    name: name.clone(),
                    message,
                };
                client.send_message(Message::new(global_chat_message));
            }
            Err(_) => break,
        }
    }

    client_stop.stop();
    client.join();
}
