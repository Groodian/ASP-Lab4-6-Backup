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
    server_socket_thread_handle: JoinHandle<()>,
    server_stop: ServerStop,
    connection_threads: Arc<Mutex<Vec<ConnectionThread>>>,
}

impl Server {
    pub fn new(connection_thread_amount: usize, address: SocketAddr) -> Self {
        // Setup the TCP server socket.
        let mut server_socket =
            TcpListener::bind(address).expect("Error while creating server socket!");

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

        // create event handler
        let (event_handler, global_chat_message_receiver, private_chat_message_event_receiver) =
            EventHandler::new(waker);

        let duration = Some(Duration::from_millis(500));

        // Next thread to add conncetion
        let mut next_thread = 0;

        let server_thread_stop = ServerThreadStop::new();

        // Server thread stops
        let mut server_thread_stops: Vec<ServerThreadStop> = Vec::new();
        server_thread_stops.push(server_thread_stop.clone());

        // Monitoring
        let mut monitoring = Monitoring::new(Duration::from_secs(30));
        let monitoring_stats = monitoring.get_new_stats();

        // create connection threads
        let connection_threads = Arc::new(Mutex::new(Vec::new()));
        let mut connection_threads_guard = connection_threads.lock().unwrap();
        for i in 0..connection_thread_amount {
            let connection_thread = ConnectionThread::new(
                format!("Thread-{}", i),
                event_handler.clone(),
                monitoring.get_new_stats(),
            );
            server_thread_stops.push(connection_thread.get_server_thread_stop());
            connection_threads_guard.push(connection_thread);
        }
        drop(connection_threads_guard);

        let server_socket_thread_handle = thread::Builder::new()
            .name(MAIN_THREAD_NAME.to_string())
            .spawn({
                let connection_threads = Arc::clone(&connection_threads);

                move || {
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
                                    // check for global chat messages

                                    let mut connection_threads_guard =
                                        connection_threads.lock().unwrap();

                                    for global_chat_message in
                                        global_chat_message_receiver.try_iter()
                                    {
                                        for connection_thread in connection_threads_guard.iter_mut()
                                        {
                                            connection_thread
                                                .event_handler
                                                .broadcast_global_chat_message(
                                                    global_chat_message.clone(),
                                                );
                                        }
                                    }

                                    // check for private messages
                                    for private_chat_message_event in
                                        private_chat_message_event_receiver.try_iter()
                                    {
                                        for connection_thread in connection_threads_guard.iter_mut()
                                        {
                                            connection_thread
                                                .event_handler
                                                .private_chat_message_event(
                                                    private_chat_message_event.clone(),
                                                );
                                        }
                                    }

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
                }
            })
            .expect(format!("Error while creating: {}", MAIN_THREAD_NAME).as_str());

        Self {
            server_socket_thread_handle,
            server_stop: ServerStop::new(server_thread_stops),
            connection_threads,
        }
    }

    pub fn get_server_stop(&self) -> ServerStop {
        self.server_stop.clone()
    }

    pub fn join(self) {
        match self.server_socket_thread_handle.join() {
            Ok(_) => println!("[{}] Stopped.", MAIN_THREAD_NAME),
            Err(_) => eprintln!("[{}] Error while stopping!", MAIN_THREAD_NAME),
        }

        // If the main thread is stopped, wait until the connection threads stopped.
        let mut connection_threads_guard = self.connection_threads.lock().unwrap();
        for connection_thread in connection_threads_guard.drain(0..) {
            connection_thread.join();
        }
        drop(connection_threads_guard);
    }
}
