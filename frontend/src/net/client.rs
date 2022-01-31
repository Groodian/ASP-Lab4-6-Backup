use crate::net::connection::Connection;
use crate::net::message::Message;
use crate::ConsoleMessage;
use mio::{net::TcpStream, Events, Interest, Poll, Token, Waker};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::{
    net::SocketAddr,
    rc::Rc,
    sync::Arc,
    thread::{self, JoinHandle},
    time::Duration,
};

const TOKEN: Token = Token(0);
const WAKER_TOKEN: Token = Token(1);

pub struct ClientStop {
    should_stop: Arc<AtomicBool>,
}

impl ClientStop {
    pub fn new() -> Self {
        Self {
            should_stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stop(&self) {
        self.should_stop.store(true, Ordering::Relaxed);
    }

    pub fn should_stop(&self) -> bool {
        return self.should_stop.load(Ordering::Relaxed);
    }
}

impl Clone for ClientStop {
    fn clone(&self) -> Self {
        Self {
            should_stop: Arc::clone(&self.should_stop),
        }
    }
}

pub struct Client {
    thread_handle: JoinHandle<()>,
    waker: Waker,
    message_sender: Sender<Message>,
    client_stop: ClientStop,
}

impl Client {
    pub fn new(address: SocketAddr, console_messages_sender: Sender<ConsoleMessage>) -> Self {
        // Create a poll instance.
        let mut poll = Poll::new().expect("Error while creating poll!");

        // Create storage for events.
        let mut events = Events::with_capacity(64);

        let duration = Some(Duration::from_millis(500));

        let std_tcp_stream =
            std::net::TcpStream::connect(address).expect("Error while connecting to server!");
        std_tcp_stream
            .set_nonblocking(true)
            .expect("Error while set non blocking!");
        let mut tcp_stream = TcpStream::from_std(std_tcp_stream);

        // Create waker instance.
        let waker = Waker::new(poll.registry(), WAKER_TOKEN).expect("Error while creating waker!");

        poll.registry()
            .register(&mut tcp_stream, TOKEN, Interest::READABLE)
            .expect("Error while registering client!");

        // create channel
        let (message_sender, message_receiver) = channel::<Message>();

        let client_stop = ClientStop::new();

        let thread_handle = thread::Builder::new()
            .name("Client".to_string())
            .spawn({
                let client_stop = client_stop.clone();
                let console_messages_sender = console_messages_sender.clone();

                move || {
                    let registry = Rc::new(
                        poll.registry()
                            .try_clone()
                            .expect("Error while clone registry!"),
                    );

                    let mut connection = Connection::new(
                        tcp_stream,
                        Rc::clone(&registry),
                        TOKEN,
                        console_messages_sender,
                    );

                    loop {
                        let poll_result = poll.poll(&mut events, duration);
                        if poll_result.is_err() {
                            eprintln!("Error while poll, retrying...");
                            continue;
                        }

                        // check if thread should stop
                        if client_stop.should_stop() {
                            return;
                        }

                        for event in events.iter() {
                            match event.token() {
                                WAKER_TOKEN => {
                                    // check for broadcast messages
                                    for message in message_receiver.try_iter() {
                                        connection.send_message(message);
                                    }
                                }
                                TOKEN => {
                                    let mut remove_connection = false;

                                    if event.is_writable() {
                                        remove_connection = connection.send();
                                    }

                                    if !remove_connection {
                                        if event.is_readable() {
                                            remove_connection = connection.read();
                                        }
                                    }

                                    if remove_connection {
                                        return;
                                    }
                                }
                                token => {
                                    // Should not happen
                                    eprintln!("Unexpected token: {}", token.0);
                                    return;
                                }
                            }
                        }
                    }
                }
            })
            .expect("Error while creating client thread!");

        Self {
            thread_handle,
            waker,
            message_sender,
            client_stop,
        }
    }

    pub fn send_message(&mut self, message: Message) {
        self.message_sender
            .send(message)
            .expect("Error while sending message!");

        self.waker.wake().expect("Error while wake!")
    }

    pub fn get_client_stop(&self) -> ClientStop {
        self.client_stop.clone()
    }

    pub fn join(self) {
        match self.thread_handle.join() {
            Ok(_) => println!("Stopped."),
            Err(_) => eprintln!("Error while stopping!"),
        }
    }
}
