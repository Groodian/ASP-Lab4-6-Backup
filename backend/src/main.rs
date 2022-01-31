use rust_chat::net::server::Server;

const THREADS_AMOUNT: usize = 4;

fn main() {
    let address = "127.0.0.1:4444"
        .parse()
        .expect("Error while parsing address!");

    let server = Server::new(THREADS_AMOUNT, address);
    let server_stop = server.get_server_stop();

    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, stopping...");
        server_stop.stop();
    })
    .expect("Error while setting Ctrl-C handler");

    server.join();
}
