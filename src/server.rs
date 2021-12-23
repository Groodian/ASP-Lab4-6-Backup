use crate::connection_thread::{self, ConnectionThread};
use mio::{net::TcpListener, Events, Interest, Poll, Token};
use std::{
    io,
    mem::take,
    net::SocketAddr,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

const SERVER_TOKEN: Token = Token(0);
const SERVER_THREAD_NAME: &str = "Main Thread";

pub struct Server {
    connection_thread_amount: usize,
    address: SocketAddr,
    should_stop: Arc<Mutex<bool>>,
    server_socket_thread_handle: Option<JoinHandle<()>>,
    connection_threads: Arc<Mutex<Vec<ConnectionThread>>>,
}

impl Server {
    pub fn new(connection_thread_amount: usize, address: SocketAddr) -> Self {
        Self {
            connection_thread_amount,
            address,
            should_stop: Arc::new(Mutex::new(false)),
            server_socket_thread_handle: None,
            connection_threads: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn start(&mut self) {
        // Setup the TCP server socket.
        let mut server_socket =
            TcpListener::bind(self.address).expect("Error while creating server socket!");

        // Create a poll instance.
        let mut poll = Poll::new().expect("Error while creating poll!");

        // Register the server with poll we can receive events for it.
        poll.registry()
            .register(&mut server_socket, SERVER_TOKEN, Interest::READABLE)
            .expect("Error while registering server socket!");

        // Create storage for events.
        let mut events = Events::with_capacity(64);

        let duration = Some(Duration::from_millis(500));

        // Next thread to add conncetion
        let mut next_thread = 0;

        let should_stop = Arc::clone(&self.should_stop);
        let connection_threads = Arc::clone(&self.connection_threads);

        let connection_thread_amount = self.connection_thread_amount.clone();

        let mut connection_threads_guard = connection_threads.lock().unwrap();
        for i in 0..self.connection_thread_amount {
            let mut connection_thread = ConnectionThread::new(format!("Thread-{}", i));
            connection_thread.start();
            connection_threads_guard.push(connection_thread);
        }
        drop(connection_threads_guard);

        self.server_socket_thread_handle = Some(
            thread::Builder::new()
                .name(SERVER_THREAD_NAME.to_string())
                .spawn(move || {
                    println!("[{}] Started.", SERVER_THREAD_NAME);

                    loop {
                        poll.poll(&mut events, duration);

                        // check if thread should stop
                        let should_stop = should_stop.lock().unwrap();
                        if *should_stop {
                            return;
                        }
                        drop(should_stop);

                        for event in events.iter() {
                            match event.token() {
                                SERVER_TOKEN => loop {
                                    // Received an event for the TCP server socket, which
                                    // indicates we can accept an connection.
                                    let (mut connection, address) = match server_socket.accept() {
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
                                                SERVER_THREAD_NAME, e
                                            );
                                            return;
                                        }
                                    };

                                    println!(
                                        "[{}] Accepted connection from: {}",
                                        SERVER_THREAD_NAME, address
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
                                token => {
                                    // Should not happen
                                    eprintln!(
                                        "[{}] Unexpected token: {}",
                                        SERVER_THREAD_NAME, token.0
                                    );
                                    return;
                                }
                            }
                        }
                    }
                })
                .expect(format!("Error while creating: {}", SERVER_THREAD_NAME).as_str()),
        );
    }

    pub fn join(&mut self) {
        let server_socket_thread_handle = take(&mut self.server_socket_thread_handle);

        match server_socket_thread_handle {
            Some(server_socket_thread_handle) => match server_socket_thread_handle.join() {
                Ok(_) => todo!(),
                Err(_) => todo!(),
            },
            None => (),
        }
    }

    pub fn stop(&mut self) {
        let mut should_stop = self.should_stop.lock().unwrap();
        *should_stop = true;
        drop(should_stop);
    }
}
