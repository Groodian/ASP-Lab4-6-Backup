use crate::net::{
    connection::Connection,
    event::EventHandler,
    monitoring::MonitoringStats,
    msg::{
        message::Message,
        messages::{GlobalChatMessage, PrivateChatMessage},
    },
    server_stop::ServerThreadStop,
};
use mio::{net::TcpStream, Events, Interest, Poll, Token, Waker};
use std::{
    collections::HashMap,
    rc::Rc,
    sync::{
        mpsc::{channel, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use super::event::PrivateChatMessageEvent;

const WAKER_TOKEN: Token = Token(0);

pub struct ConnectionThread {
    connection_thread_name: String,
    server_thread_stop: ServerThreadStop,
    waker: Arc<Waker>,
    pub event_handler: EventHandler,
    new_connection_sender: Sender<TcpStream>,
    connection_thread_handle: JoinHandle<()>,
}

impl ConnectionThread {
    pub fn new(
        connection_thread_name: String,
        server_event_handler: EventHandler,
        monitoring_stats: Arc<MonitoringStats>,
    ) -> Self {
        // Create a poll instance.
        let mut poll = Poll::new().expect("Error while creating poll!");

        // Create waker connection instance.
        let waker = Arc::new(
            Waker::new(poll.registry(), WAKER_TOKEN).expect("Error while creating waker!"),
        );

        // create channels
        let (new_connection_sender, new_connection_receiver) = channel::<TcpStream>();
        let (global_chat_message_sender, global_chat_message_receiver) =
            channel::<GlobalChatMessage>();
        let (private_chat_message_event_sender, private_chat_message_event_receiver) =
            channel::<PrivateChatMessageEvent>();

        // create event handler
        let event_handler = EventHandler::new(
            Arc::clone(&waker),
            global_chat_message_sender,
            private_chat_message_event_sender,
        );

        let server_thread_stop = ServerThreadStop::new();

        let connection_thread_handle = thread::Builder::new()
            .name(connection_thread_name.to_string())
            .spawn({
                let connection_thread_name = connection_thread_name.clone();
                let server_event_handler = server_event_handler.clone();
                let server_thread_stop = server_thread_stop.clone();

                move || {
                    // Map of `Token` -> `TcpStream`.
                    let mut connections: HashMap<Token, Connection> = HashMap::new();

                    // Create storage for events.
                    let mut events = Events::with_capacity(128);

                    // Unique token for each incoming connection.
                    let mut next_token = Token(2);

                    let duration = Some(Duration::from_millis(500));

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
                                    for mut new_connection in new_connection_receiver.try_iter() {
                                        poll.registry()
                                            .register(
                                                &mut new_connection,
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
                                                new_connection,
                                                server_event_handler.clone(),
                                                Arc::clone(&monitoring_stats),
                                                Rc::clone(&registry),
                                                next_token,
                                            ),
                                        );

                                        next_token = Token(next_token.0 + 1);
                                    }

                                    // check for global chat messages
                                    for global_chat_message in
                                        global_chat_message_receiver.try_iter()
                                    {
                                        let message = Message::new(global_chat_message);
                                        for connection in connections.iter_mut() {
                                            connection.1.send_message(message.clone());
                                        }
                                    }

                                    // check for private messages
                                    for private_chat_message_event in
                                        private_chat_message_event_receiver.try_iter()
                                    {
                                        let message = Message::new(PrivateChatMessage {
                                            from_user_name: private_chat_message_event
                                                .from_user_name,
                                            message: private_chat_message_event.message,
                                        });
                                        for connection in connections.iter_mut() {
                                            match &connection.1.user_name {
                                                Some(user_name) => {
                                                    if user_name
                                                        == &private_chat_message_event.to_user_name
                                                    {
                                                        connection.1.send_message(message.clone());
                                                    }
                                                }
                                                None => {}
                                            }
                                        }
                                    }
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
                                            println!("Connection closed.");
                                            monitoring_stats.lost_connection();

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
                }
            })
            .expect(format!("Error while creating: {}", connection_thread_name).as_str());

        Self {
            connection_thread_name,
            server_thread_stop,
            waker,
            event_handler,
            new_connection_sender,
            connection_thread_handle,
        }
    }

    pub fn add_connection(&self, connection: TcpStream) {
        self.new_connection_sender
            .send(connection)
            .expect("Error while adding connection!");

        self.waker.wake().expect("Error while wake!")
    }

    pub fn get_server_thread_stop(&self) -> ServerThreadStop {
        self.server_thread_stop.clone()
    }

    pub fn join(self) {
        match self.connection_thread_handle.join() {
            Ok(_) => println!("[{}] Stopped.", self.connection_thread_name),
            Err(_) => eprintln!("[{}] Error while stopping!", self.connection_thread_name),
        }
    }
}
