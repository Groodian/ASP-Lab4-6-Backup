pub mod net;

use crate::net::server::Server;

const THREADS_AMOUNT: usize = 4;

fn main() {
    let address = "127.0.0.1:9000"
        .parse()
        .expect("Error while parsing address!");

    let mut server = Server::new(THREADS_AMOUNT, address);
    let server_stop = server.get_server_stop();

    server.start();

    ctrlc::set_handler(move || {
        println!("Received Ctrl+C, stopping...");
        server_stop.stop();
    })
    .expect("Error while setting Ctrl-C handler");

    server.join();
}
