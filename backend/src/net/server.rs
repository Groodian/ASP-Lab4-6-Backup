use crate::net::{
    connection_thread::ConnectionThread,
    event::EventHandler,
    monitoring::Monitoring,
    server_stop::{ServerStop, ServerThreadStop},
};
use mio::{net::TcpListener, Events, Interest, Poll, Token, Waker};
use std::{
    io,
    net::SocketAddr,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

const SERVER_SOCKET_TOKEN: Token = Token(0);
const WAKER_TOKEN_BROADCAST: Token = Token(1);
const MAIN_THREAD_NAME: &str = "Thread-Main";

pub struct Server {
    connection_thread_amount: usize,
    address: SocketAddr,
    server_thread_stop: ServerThreadStop,
    server_socket_thread_handle: Option<JoinHandle<()>>,
    connection_threads: Arc<Mutex<Vec<ConnectionThread>>>,
}

impl Server {
    pub fn new(connection_thread_amount: usize, address: SocketAddr) -> Self {
        Self {
            connection_thread_amount,
            address,
            server_thread_stop: ServerThreadStop::new(),
            server_socket_thread_handle: None,
            connection_threads: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn start(&mut self) -> ServerStop {
        // Setup the TCP server socket.
        let mut server_socket =
            TcpListener::bind(self.address).expect("Error while creating server socket!");

        // Create a poll instance.
        let mut poll = Poll::new().expect("Error while creating poll!");

        // Register the server with poll we can receive events for it.
        poll.registry()
            .register(&mut server_socket, SERVER_SOCKET_TOKEN, Interest::READABLE)
            .expect("Error while registering server socket!");

        // Create storage for events.
        let mut events = Events::with_capacity(64);

        // Create waker broadcast instance.
        let waker = Arc::new(
            Waker::new(poll.registry(), WAKER_TOKEN_BROADCAST)
                .expect("Error while creating waker!"),
        );

        let event_handler = EventHandler::new(Some(waker));

        let duration = Some(Duration::from_millis(500));

        // Next thread to add conncetion
        let mut next_thread = 0;

        // Server thread stops
        let mut server_thread_stops: Vec<ServerThreadStop> = Vec::new();
        server_thread_stops.push(self.server_thread_stop.clone());

        let server_thread_stop = self.server_thread_stop.clone();
        let connection_threads = Arc::clone(&self.connection_threads);

        let connection_thread_amount = self.connection_thread_amount.clone();

        // Monitoring
        let mut monitoring = Monitoring::new(Duration::from_secs(30));
        let monitoring_stats = monitoring.get_new_stats();

        let mut connection_threads_guard = connection_threads.lock().unwrap();
        for i in 0..self.connection_thread_amount {
            let mut connection_thread = ConnectionThread::new(
                format!("Thread-{}", i),
                event_handler.clone(),
                monitoring.get_new_stats(),
            );
            connection_thread.start();
            server_thread_stops.push(connection_thread.get_server_thread_stop());
            connection_threads_guard.push(connection_thread);
        }
        drop(connection_threads_guard);

        self.server_socket_thread_handle = Some(
            thread::Builder::new()
                .name(MAIN_THREAD_NAME.to_string())
                .spawn(move || {
                    println!("[{}] Started.", MAIN_THREAD_NAME);

                    loop {
                        let poll_result = poll.poll(&mut events, duration);
                        if poll_result.is_err() {
                            eprintln!("[{}] Error while poll, retrying...", MAIN_THREAD_NAME);
                            continue;
                        }

                        // check if thread should stop
                        if server_thread_stop.should_stop() {
                            return;
                        }

                        monitoring.update();

                        for event in events.iter() {
                            match event.token() {
                                SERVER_SOCKET_TOKEN => loop {
                                    // Received an event for the TCP server socket, which
                                    // indicates we can accept an connection.
                                    let (connection, address) = match server_socket.accept() {
                                        Ok((connection, address)) => (connection, address),
                                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                            // If we get a `WouldBlock` error we know our
                                            // listener has no more incoming connections queued,
                                            // so we can return to polling and wait for some
                                            // more.
                                            break;
                                        }
                                        Err(e) => {
                                            // If it was any other kind of error, something went
                                            // wrong and we terminate with an error.
                                            eprintln!(
                                                "[{}] Unexpected error: {}",
                                                MAIN_THREAD_NAME, e
                                            );
                                            return;
                                        }
                                    };

                                    monitoring_stats.new_connection();

                                    println!(
                                        "[{}] Accepted connection from: {} and moved to Thread: {}",
                                        MAIN_THREAD_NAME, address, next_thread
                                    );

                                    let connection_threads = connection_threads.lock().unwrap();

                                    match connection_threads.get(next_thread) {
                                        Some(connection_thread) => {
                                            connection_thread.add_connection(connection);
                                        }
                                        None => (),
                                    }

                                    next_thread += 1;
                                    if next_thread >= connection_thread_amount {
                                        next_thread = 0;
                                    }

                                    drop(connection_threads);
                                },
                                WAKER_TOKEN_BROADCAST => {
                                    println!("test2");
                                    let mut connection_threads_guard =
                                        connection_threads.lock().unwrap();

                                    // check for global chat messages
                                    let mut global_chat_message_queue_guard =
                                        event_handler.global_chat_message_queue.lock().unwrap();
                                    loop {
                                        match global_chat_message_queue_guard.pop_front() {
                                            Some(message) => {
                                                for connection_thread in
                                                    connection_threads_guard.iter_mut()
                                                {
                                                    connection_thread
                                                        .event_handler
                                                        .broadcast_global_chat_message(
                                                            message.clone(),
                                                        );
                                                }
                                            }
                                            None => break,
                                        }
                                    }
                                    drop(global_chat_message_queue_guard);

                                    // check for private messages
                                    let mut private_chat_message_event_queue_guard = event_handler
                                        .private_chat_message_event_queue
                                        .lock()
                                        .unwrap();
                                    loop {
                                        match private_chat_message_event_queue_guard.pop_front() {
                                            Some(message) => {
                                                for connection_thread in
                                                    connection_threads_guard.iter_mut()
                                                {
                                                    connection_thread
                                                        .event_handler
                                                        .private_chat_message_event(
                                                            message.clone(),
                                                        );
                                                }
                                            }
                                            None => break,
                                        }
                                    }
                                    drop(private_chat_message_event_queue_guard);

                                    drop(connection_threads_guard);
                                }
                                token => {
                                    // Should not happen
                                    eprintln!(
                                        "[{}] Unexpected token: {}",
                                        MAIN_THREAD_NAME, token.0
                                    );
                                    return;
                                }
                            }
                        }
                    }
                })
                .expect(format!("Error while creating: {}", MAIN_THREAD_NAME).as_str()),
        );

        ServerStop::new(server_thread_stops)
    }

    pub fn join(&mut self) {
        match self.server_socket_thread_handle.take() {
            Some(server_socket_thread_handle) => match server_socket_thread_handle.join() {
                Ok(_) => println!("[{}] Stopped.", MAIN_THREAD_NAME),
                Err(_) => eprintln!("[{}] Error while stopping!", MAIN_THREAD_NAME),
            },
            None => (),
        }

        // If the main thread is stopped, wait until the connection threads stopped.
        let mut connection_threads_guard = self.connection_threads.lock().unwrap();
        for connection_thread in connection_threads_guard.iter_mut() {
            connection_thread.join();
        }
        drop(connection_threads_guard);
    }
}
