use crate::net::{connection::Connection, msg::message::Message, server_stop::ServerThreadStop, server::ServerBroadcastMessage, monitoring::MonitoringStats};
use mio::{net::TcpStream, Events, Interest, Poll, Token, Waker};
use std::{
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

const WAKER_TOKEN: Token = Token(0);

pub struct ConnectionThread {
    connection_thread_name: String,
    server_broadcast_message: ServerBroadcastMessage,
    monitoring_stats: Arc<MonitoringStats>,
    server_thread_stop: ServerThreadStop,
    waker: Option<Arc<Waker>>,
    new_connections: Arc<Mutex<VecDeque<TcpStream>>>,
    broadcast_messages: Arc<Mutex<VecDeque<Message>>>,
    connection_thread_handle: Option<JoinHandle<()>>,
}

impl ConnectionThread {
    pub fn new(connection_thread_name: String, server_broadcast_message: ServerBroadcastMessage, monitoring_stats: Arc<MonitoringStats>) -> Self {
        Self {
            connection_thread_name,
            server_broadcast_message,
            monitoring_stats,
            server_thread_stop: ServerThreadStop::new(),
            waker: None,
            new_connections: Arc::new(Mutex::new(VecDeque::new())),
            broadcast_messages: Arc::new(Mutex::new(VecDeque::new())),
            connection_thread_handle: None,
        }
    }

    pub fn start(&mut self) {
        // Create a poll instance.
        let mut poll = Poll::new().expect("Error while creating poll!");

        // Create storage for events.
        let mut events = Events::with_capacity(64);

        // Create waker connection instance.
        self.waker = Some(Arc::new(
            Waker::new(poll.registry(), WAKER_TOKEN)
                .expect("Error while creating waker!"),
        ));

        // Unique token for each incoming connection.
        let mut next_token = Token(2);

        let duration = Some(Duration::from_millis(500));

        let server_thread_stop = self.server_thread_stop.clone();
        let new_connections = Arc::clone(&self.new_connections);
        let broadcast_messages = Arc::clone(&self.broadcast_messages);
        let server_broadcast_message = self.server_broadcast_message.clone();
        let monitoring_stats = Arc::clone(&self.monitoring_stats);

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
                                WAKER_TOKEN => {
                                    // check for new connections
                                    let mut new_connections = new_connections.lock().unwrap();

                                    loop {

                                        match  new_connections.pop_front(){
                                            Some(connection) => {
                                                let mut connection = connection;
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
                                                    server_broadcast_message.clone(),
                                                    Arc::clone(&monitoring_stats),
                                                    Rc::clone(&registry),
                                                    next_token,
                                                ),
                                            );
    
                                            next_token = Token(next_token.0 + 1);
                                            },
                                            None => break,
                                        } 


                                    }

                                    drop(new_connections);

                                    // check for broadcast messages
                                    let mut broadcast_messages = broadcast_messages.lock().unwrap();

                                    loop {
                                        match broadcast_messages.pop_front() {
                                            Some(message) => {
                                                for connection in connections.iter_mut() {
                                                    connection.1.send_message(message.clone());
                                                }
                                            },
                                            None => break,
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
                                        if let Some(connection) = connections.remove(&token) {
                                            poll.registry()
                                                .deregister(&mut connection.tcp_stream())
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

        new_connections.push_back(connection);

        drop(new_connections);

        match &self.waker {
            Some(waker) => waker.wake().expect("Error while wake!"),
            None => (),
        }
    }

    pub fn broadcast_message(&self, message: Message) {
        let mut broadcast_messages = self.broadcast_messages.lock().unwrap();

        broadcast_messages.push_back(message);

        drop(broadcast_messages);

        match &self.waker {
            Some(waker) => waker.wake().expect("Error while wake!"),
            None => (),
        }
    }

    pub fn join(&mut self) {
        match self.connection_thread_handle.take() {
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
