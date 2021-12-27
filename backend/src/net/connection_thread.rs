use crate::net::connection::Connection;
use crate::net::server_stop::ServerThreadStop;
use mio::{event::Event, net::TcpStream, Events, Interest, Poll, Registry, Token, Waker};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    mem::take,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

const WAKER_TOKEN: Token = Token(0);
const DATA: &[u8] = b"Hello world!\n";

pub struct ConnectionThread {
    connection_thread_name: String,
    server_thread_stop: ServerThreadStop,
    waker: Option<Arc<Waker>>,
    new_connections: Arc<Mutex<Vec<TcpStream>>>,
    connection_thread_handle: Option<JoinHandle<()>>,
}

impl ConnectionThread {
    pub fn new(connection_thread_name: String) -> Self {
        Self {
            connection_thread_name,
            server_thread_stop: ServerThreadStop::new(),
            waker: None,
            new_connections: Arc::new(Mutex::new(Vec::new())),
            connection_thread_handle: None,
        }
    }

    pub fn start(&mut self) {
        // Create a poll instance.
        let mut poll = Poll::new().expect("Error while creating poll!");

        // Create storage for events.
        let mut events = Events::with_capacity(64);

        // Map of `Token` -> `TcpStream`.
        let mut connections: HashMap<Token, Connection> = HashMap::new();

        // Create waker instance.
        self.waker = Some(Arc::new(
            Waker::new(poll.registry(), WAKER_TOKEN).expect("Error while creating waker!"),
        ));

        // Unique token for each incoming connection.
        let mut unique_token = Token(WAKER_TOKEN.0 + 1);

        let duration = Some(Duration::from_millis(500));

        let server_thread_stop = self.server_thread_stop.clone();
        let new_connections = Arc::clone(&self.new_connections);

        let connection_thread_name = self.connection_thread_name.clone();

        self.connection_thread_handle = Some(
            thread::Builder::new()
                .name(connection_thread_name.to_string())
                .spawn(move || {
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
                                    let mut new_connections = new_connections.lock().unwrap();

                                    for _ in 0..new_connections.len() {
                                        let mut connection = new_connections.remove(0);
                                        let token = ConnectionThread::next(&mut unique_token);

                                        poll.registry()
                                            .register(
                                                &mut connection,
                                                token,
                                                Interest::READABLE,
                                            )
                                            .expect(format!("[{}] Error while registering new connection!", connection_thread_name).as_str());

                                        connections.insert(token, Connection::new(connection));
                                    }

                                    drop(new_connections);
                                }
                                token => {
                                    // Maybe received an event for a TCP connection.
                                    let done = if let Some(connection) = connections.get_mut(&token)
                                    {
                                        ConnectionThread::handle_connection_event(
                                            connection_thread_name.as_str(),
                                            poll.registry(),
                                            connection,
                                            event,
                                        )
                                    } else {
                                        // Sporadic events happen, we can safely ignore them.
                                        Ok(false)
                                    };

                                    match done {
                                        Ok(ok) => {
                                            if ok {
                                                if let Some(mut connection) = connections.remove(&token) {
                                                    poll.registry()
                                                        .deregister(&mut connection.connection)
                                                        .expect(format!("[{}] Error while deregister connection!", connection_thread_name).as_str());
                                                }
                                            }
                                        }
                                        Err(_) => (),
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

        match &self.waker {
            Some(waker) => waker.wake().expect("Error while wake!"),
            None => (),
        }
    }

    /// Returns `true` if the connection is done.
    fn handle_connection_event(
        connection_thread_name: &str,
        registry: &Registry,
        connection: &mut Connection,
        event: &Event,
    ) -> io::Result<bool> {
        if event.is_writable() {
            // We can (maybe) write to the connection.
            match connection.connection.write(DATA) {
                // We want to write the entire `DATA` buffer in a single go. If we
                // write less we'll return a short write error (same as
                // `io::Write::write_all` does).
                Ok(n) if n < DATA.len() => return Err(io::ErrorKind::WriteZero.into()),
                Ok(_) => {
                    // After we've written something we'll reregister the connection
                    // to only respond to readable events.
                    registry.reregister(
                        &mut connection.connection,
                        event.token(),
                        Interest::READABLE,
                    )?
                }
                // Would block "errors" are the OS's way of saying that the
                // connection is not actually ready to perform this I/O operation.
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {}
                // Got interrupted (how rude!), we'll try again.
                Err(ref err) if err.kind() == io::ErrorKind::Interrupted => {
                    return ConnectionThread::handle_connection_event(
                        connection_thread_name,
                        registry,
                        connection,
                        event,
                    )
                }
                // Other errors we'll consider fatal.
                Err(err) => return Err(err),
            }
        }

        if event.is_readable() {
            // We can (maybe) read from the connection.
            loop {
                match connection.connection.read(&mut connection.message_factory.buffer[connection.message_factory.buffer_pos..]) {
                    Ok(0) => {
                        // Reading 0 bytes means the other side has closed the
                        // connection or is done writing, then so are we.
                        println!("[{}] Connection closed.", connection_thread_name);
                        return Ok(true);
                    }
                    Ok(n) => {
                        connection.message_factory.buffer_pos += n;

                        if !connection.message_factory.decode() {
                            println!("[{}] Invalid data, closing connection...", connection_thread_name);
                            return Ok(true);
                        }
                    }
                    // Would block "errors" are the OS's way of saying that the
                    // connection is not actually ready to perform this I/O operation.
                    Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => break,
                    Err(ref err) if err.kind() == io::ErrorKind::Interrupted => continue,
                    // Other errors we'll consider fatal.
                    Err(err) => return Err(err),
                }
            }
        }

        Ok(false)
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

    fn next(current: &mut Token) -> Token {
        let next = current.0;
        current.0 += 1;
        Token(next)
    }
}
