use crate::net::{connection::Connection, msg::msgs::Message, server_stop::ServerThreadStop};
use mio::{net::TcpStream, Events, Interest, Poll, Token, Waker};
use std::{
    collections::HashMap,
    mem::take,
    rc::Rc,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

const WAKER_TOKEN_CONNECTION: Token = Token(0);
const WAKER_TOKEN_BROADCAST: Token = Token(1);

pub struct ConnectionThread {
    connection_thread_name: String,
    server_thread_stop: ServerThreadStop,
    waker_connection: Option<Arc<Waker>>,
    waker_broadcast: Option<Arc<Waker>>,
    new_connections: Arc<Mutex<Vec<TcpStream>>>,
    broadcast_messages: Arc<Mutex<Vec<Message>>>,
    connection_thread_handle: Option<JoinHandle<()>>,
}

impl ConnectionThread {
    pub fn new(connection_thread_name: String) -> Self {
        Self {
            connection_thread_name,
            server_thread_stop: ServerThreadStop::new(),
            waker_connection: None,
            waker_broadcast: None,
            new_connections: Arc::new(Mutex::new(Vec::new())),
            broadcast_messages: Arc::new(Mutex::new(Vec::new())),
            connection_thread_handle: None,
        }
    }

    pub fn start(&mut self) {
        // Create a poll instance.
        let mut poll = Poll::new().expect("Error while creating poll!");

        // Create storage for events.
        let mut events = Events::with_capacity(64);

        // Create waker connection instance.
        self.waker_connection = Some(Arc::new(
            Waker::new(poll.registry(), WAKER_TOKEN_CONNECTION)
                .expect("Error while creating waker!"),
        ));

        // Create waker broadcast instance.
        self.waker_broadcast = Some(Arc::new(
            Waker::new(poll.registry(), WAKER_TOKEN_BROADCAST)
                .expect("Error while creating waker!"),
        ));

        // Unique token for each incoming connection.
        let mut next_token = Token(2);

        let duration = Some(Duration::from_millis(500));

        let server_thread_stop = self.server_thread_stop.clone();
        let new_connections = Arc::clone(&self.new_connections);
        let broadcast_messages = Arc::clone(&self.broadcast_messages);

        let connection_thread_name = self.connection_thread_name.clone();

        self.connection_thread_handle = Some(
            thread::Builder::new()
                .name(connection_thread_name.to_string())
                .spawn(move || {
                    // Map of `Token` -> `TcpStream`.
                    let mut connections: HashMap<Token, Connection> = HashMap::new();

                    let registry = Rc::new(
                        poll.registry()
                            .try_clone()
                            .expect("Error while clone registry!"),
                    );
                    println!("[{}] Started.", connection_thread_name);

                    loop {
                        let poll_result = poll.poll(&mut events, duration);
                        if poll_result.is_err() {
                            eprintln!("[{}] Error while poll, retrying...", connection_thread_name);
                            continue;
                        }

                        // check if thread should stop
                        if server_thread_stop.should_stop() {
                            return;
                        }

                        for event in events.iter() {
                            match event.token() {
                                WAKER_TOKEN_CONNECTION => {
                                    let mut new_connections = new_connections.lock().unwrap();

                                    for _ in 0..new_connections.len() {
                                        let mut connection = new_connections.remove(0);

                                        poll.registry()
                                            .register(
                                                &mut connection,
                                                next_token,
                                                Interest::READABLE,
                                            )
                                            .expect(
                                                format!(
                                                    "[{}] Error while registering new connection!",
                                                    connection_thread_name
                                                )
                                                .as_str(),
                                            );

                                        connections.insert(
                                            next_token,
                                            Connection::new(
                                                connection,
                                                Rc::clone(&registry),
                                                next_token,
                                            ),
                                        );

                                        next_token = Token(next_token.0 + 1);
                                    }

                                    drop(new_connections);
                                }
                                WAKER_TOKEN_BROADCAST => {
                                    let mut broadcast_messages = broadcast_messages.lock().unwrap();

                                    for _ in 0..broadcast_messages.len() {
                                        let message = broadcast_messages.remove(0);

                                        for connection in connections.iter_mut() {
                                            connection.1.send_message(message.clone());
                                        }
                                    }

                                    drop(broadcast_messages);
                                }
                                token => {
                                    // Maybe received an event for a TCP connection.
                                    let mut remove_connection = false;

                                    match connections.get_mut(&token) {
                                        Some(connection) => {
                                            if event.is_writable() {
                                                remove_connection = connection.send();
                                            }

                                            if !remove_connection {
                                                if event.is_readable() {
                                                    remove_connection = connection.read();
                                                }
                                            }
                                        }
                                        // Sporadic events happen, we can safely ignore them.
                                        None => {}
                                    }

                                    if remove_connection {
                                        if let Some(mut connection) = connections.remove(&token) {
                                            poll.registry()
                                                .deregister(&mut connection.tcp_stream)
                                                .expect(
                                                    format!(
                                                        "[{}] Error while deregister connection!",
                                                        connection_thread_name
                                                    )
                                                    .as_str(),
                                                );
                                        }
                                    }
                                }
                            }
                        }
                    }
                })
                .expect(format!("Error while creating: {}", self.connection_thread_name).as_str()),
        );
    }

    pub fn add_connection(&self, connection: TcpStream) {
        let mut new_connections = self.new_connections.lock().unwrap();

        new_connections.push(connection);

        drop(new_connections);

        match &self.waker_connection {
            Some(waker) => waker.wake().expect("Error while wake!"),
            None => (),
        }
    }

    pub fn broadcast_message(&self, message: Message) {
        let mut broadcast_messages = self.broadcast_messages.lock().unwrap();

        broadcast_messages.push(message);

        drop(broadcast_messages);

        match &self.waker_broadcast {
            Some(waker) => waker.wake().expect("Error while wake!"),
            None => (),
        }
    }

    pub fn join(&mut self) {
        let connection_thread_handle = take(&mut self.connection_thread_handle);

        match connection_thread_handle {
            Some(connection_thread_handle) => match connection_thread_handle.join() {
                Ok(_) => println!("[{}] Stopped.", self.connection_thread_name),
                Err(_) => eprintln!("[{}] Error while stopping!", self.connection_thread_name),
            },
            None => (),
        }
    }

    pub fn get_server_thread_stop(&self) -> ServerThreadStop {
        self.server_thread_stop.clone()
    }
}
