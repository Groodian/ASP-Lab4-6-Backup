mod connection_thread;
mod server;

use crate::server::Server;

const THREADS_AMOUNT: usize = 4;

fn main() {
    let address = "127.0.0.1:9000"
        .parse()
        .expect("Error while parsing address!");

    let mut server = Server::new(THREADS_AMOUNT, address);
    server.start();

    /*
        ctrlc::set_handler(move || {
            println!("received Ctrl+C!");
            server.stop();
        })
        .expect("Error while setting Ctrl-C handler");

    */

    server.join();
}
