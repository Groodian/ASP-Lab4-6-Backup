use crate::net::connection::Connection;
use crate::net::message::Message;
use mio::{net::TcpStream, Events, Interest, Poll, Token, Waker};
use std::collections::VecDeque;
use std::sync::Mutex;
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
    waker: Arc<Waker>,
}

impl ClientStop {
    pub fn new(waker: Arc<Waker>) -> Self {
        Self { waker }
    }

    pub fn stop(&self) {
        self.waker.wake().expect("Error while wake!");
    }
}

impl Clone for ClientStop {
    fn clone(&self) -> Self {
        Self {
            waker: Arc::clone(&self.waker),
        }
    }
}

pub struct Client {
    address: SocketAddr,
    thread_handle: Option<JoinHandle<()>>,
    waker: Option<Arc<Waker>>,
    messages_queue: Arc<Mutex<VecDeque<Message>>>,
}

impl Client {
    pub fn new(address: SocketAddr) -> Self {
        Self {
            address,
            thread_handle: None,
            waker: None,
            messages_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn connect(&mut self) -> ClientStop {
        // Create a poll instance.
        let mut poll = Poll::new().expect("Error while creating poll!");

        // Create storage for events.
        let mut events = Events::with_capacity(64);

        let duration = Some(Duration::from_millis(500));

        let std_tcp_stream =
            std::net::TcpStream::connect(self.address).expect("Error while connecting to server!");
        std_tcp_stream
            .set_nonblocking(true)
            .expect("Error while set non blocking!");
        let mut tcp_stream = TcpStream::from_std(std_tcp_stream);

        // Create waker instance.
        let waker = Arc::new(
            Waker::new(poll.registry(), WAKER_TOKEN).expect("Error while creating waker!"),
        );
        self.waker = Some(waker.clone());

        poll.registry()
            .register(&mut tcp_stream, TOKEN, Interest::READABLE)
            .expect("Error while registering client!");

        let messages_queue = Arc::clone(&self.messages_queue);

        self.thread_handle = Some(
            thread::Builder::new()
                .name("Client".to_string())
                .spawn(move || {
                    let registry = Rc::new(
                        poll.registry()
                            .try_clone()
                            .expect("Error while clone registry!"),
                    );

                    let mut connection = Connection::new(tcp_stream, Rc::clone(&registry), TOKEN);

                    loop {
                        let poll_result = poll.poll(&mut events, duration);
                        if poll_result.is_err() {
                            eprintln!("Error while poll, retrying...");
                            continue;
                        }

                        for event in events.iter() {
                            match event.token() {
                                WAKER_TOKEN => {
                                    // check for broadcast messages
                                    let mut messages_queue = messages_queue.lock().unwrap();

                                    loop {
                                        match messages_queue.pop_front() {
                                            Some(message) => {
                                                connection.send_message(message);
                                            }
                                            None => break,
                                        }
                                    }

                                    drop(messages_queue);
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
                })
                .expect("Error while creating client thread!"),
        );

        ClientStop::new(waker)
    }

    pub fn send_message(&mut self, message: Message) {
        let mut broadcast_messages = self.messages_queue.lock().unwrap();

        broadcast_messages.push_back(message);

        drop(broadcast_messages);

        match &self.waker {
            Some(waker) => waker.wake().expect("Error while wake!"),
            None => (),
        }
    }

    pub fn join(self) {
        match self.thread_handle {
            Some(thread_handle) => match thread_handle.join() {
                Ok(_) => println!("Stopped."),
                Err(_) => eprintln!("Error while stopping!"),
            },
            None => (),
        }
    }
}