use crate::net::server_stop::ServerStop;
use mio::{event::Event, net::TcpStream, Events, Interest, Poll, Registry, Token, Waker};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    mem::take,
    str::from_utf8,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

const WAKER_TOKEN: Token = Token(0);
const DATA: &[u8] = b"Hello world!\n";

pub struct ConnectionThread {
    connection_thread_name: String,
    server_stop: ServerStop,
    waker: Option<Arc<Waker>>,
    new_connections: Arc<Mutex<Vec<TcpStream>>>,
    connection_thread_handle: Option<JoinHandle<()>>,
}

impl ConnectionThread {
    pub fn new(connection_thread_name: String) -> Self {
        Self {
            connection_thread_name,
            server_stop: ServerStop::new(),
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
        let mut connections: HashMap<Token, TcpStream> = HashMap::new();

        // Create waker instance.
        self.waker = Some(Arc::new(
            Waker::new(poll.registry(), WAKER_TOKEN).expect("Error while creating waker!"),
        ));

        // Unique token for each incoming connection.
        let mut unique_token = Token(WAKER_TOKEN.0 + 1);

        let duration = Some(Duration::from_millis(500));

        let server_stop = self.server_stop.clone();
        let new_connections = Arc::clone(&self.new_connections);

        let connection_thread_name = self.connection_thread_name.clone();

        self.connection_thread_handle = Some(
            thread::Builder::new()
                .name(connection_thread_name.to_string())
                .spawn(move || {
                    println!("[{}] Started.", connection_thread_name);

                    loop {
                        poll.poll(&mut events, duration);

                        // check if thread should stop
                        if server_stop.should_stop() {
                            return;
                        }

                        for event in events.iter() {
                            match event.token() {
                                WAKER_TOKEN => {
                                    let mut new_connections = new_connections.lock().unwrap();

                                    for _ in 0..new_connections.len() {
                                        let mut connection = new_connections.remove(0);
                                        let token = ConnectionThread::next(&mut unique_token);

                                        poll.registry().register(
                                            &mut connection,
                                            token,
                                            Interest::READABLE.add(Interest::WRITABLE),
                                        );

                                        connections.insert(token, connection);
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
                                                if let Some(mut connection) =
                                                    connections.remove(&token)
                                                {
                                                    poll.registry().deregister(&mut connection);
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
        connection: &mut TcpStream,
        event: &Event,
    ) -> io::Result<bool> {
        if event.is_writable() {
            // We can (maybe) write to the connection.
            match connection.write(DATA) {
                // We want to write the entire `DATA` buffer in a single go. If we
                // write less we'll return a short write error (same as
                // `io::Write::write_all` does).
                Ok(n) if n < DATA.len() => return Err(io::ErrorKind::WriteZero.into()),
                Ok(_) => {
                    // After we've written something we'll reregister the connection
                    // to only respond to readable events.
                    registry.reregister(connection, event.token(), Interest::READABLE)?
                }
                // Would block "errors" are the OS's way of saying that the
                // connection is not actually ready to perform this I/O operation.
                Err(ref err) if ConnectionThread::would_block(err) => {}
                // Got interrupted (how rude!), we'll try again.
                Err(ref err) if ConnectionThread::interrupted(err) => {
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
            let mut connection_closed = false;
            let mut received_data = vec![0; 4096];
            let mut bytes_read = 0;
            // We can (maybe) read from the connection.
            loop {
                match connection.read(&mut received_data[bytes_read..]) {
                    Ok(0) => {
                        // Reading 0 bytes means the other side has closed the
                        // connection or is done writing, then so are we.
                        connection_closed = true;
                        break;
                    }
                    Ok(n) => {
                        bytes_read += n;
                        if bytes_read == received_data.len() {
                            received_data.resize(received_data.len() + 1024, 0);
                        }
                    }
                    // Would block "errors" are the OS's way of saying that the
                    // connection is not actually ready to perform this I/O operation.
                    Err(ref err) if ConnectionThread::would_block(err) => break,
                    Err(ref err) if ConnectionThread::interrupted(err) => continue,
                    // Other errors we'll consider fatal.
                    Err(err) => return Err(err),
                }
            }

            if bytes_read != 0 {
                let received_data = &received_data[..bytes_read];
                if let Ok(str_buf) = from_utf8(received_data) {
                    println!(
                        "[{}] Received data: {}",
                        connection_thread_name,
                        str_buf.trim_end()
                    );
                } else {
                    println!(
                        "[{}] Received (none UTF-8) data: {:?}",
                        connection_thread_name, received_data
                    );
                }
            }

            if connection_closed {
                println!("[{}] Connection closed", connection_thread_name);
                return Ok(true);
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

    pub fn get_server_stop(&self) -> ServerStop {
        self.server_stop.clone()
    }

    fn next(current: &mut Token) -> Token {
        let next = current.0;
        current.0 += 1;
        Token(next)
    }

    fn would_block(err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::WouldBlock
    }

    fn interrupted(err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::Interrupted
    }
}
