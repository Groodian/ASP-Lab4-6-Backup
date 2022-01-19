use crate::net::message::{Message, MessageTrait};
use crate::net::messages::{
    GlobalChatMessage, LoginMessage, PingMessage, PrivateChatMessage, PublishGlobalChatMessage,
    PublishPrivateChatMessage,
};
use mio::{net::TcpStream, Interest, Registry, Token};
use std::rc::Rc;
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    str::from_utf8,
};

/*
    Message Header:
    bytes   name               description
    8       magic number       Magic number to identify the application
    3       message number     Message number to identify the message type
    5       payload size       Payload size, maximum is MAX_PAYLOAD
    total size: 16 bytes
*/

const MAGIC: &[u8; 8] = b"RustChat";
const HEADER_SIZE: usize = 16;
const MAX_PAYLOAD: usize = 1024;
const MAX_PACKET_SIZE: usize = HEADER_SIZE + MAX_PAYLOAD;

macro_rules! ProcessMessage {
    ($message_struct:ident, $utf8_payload:expr, $connection:expr) => {
        match serde_json::from_str::<$message_struct>($utf8_payload) {
            Ok(message) => message.process($connection),
            Err(_) => return false,
        }
    };
}

#[derive(PartialEq, Eq)]
enum MessageDecodeState {
    WaitingForHeader,
    HeaderSuccessfulRead,
    WaitingForPayload,
    PayloadSuccessfulRead,
}

pub struct Connection {
    pub tcp_stream: TcpStream,
    registry: Rc<Registry>,
    token: Token,
    message_queue: VecDeque<Message>, // has no limit!!!
    out_buffer: Box<[u8; MAX_PAYLOAD]>,
    out_buffer_pos: usize,
    out_buffer_size: usize,
    in_buffer: Box<[u8; MAX_PACKET_SIZE]>,
    in_buffer_pos: usize,
    send_interest: bool,
    // current message decode data/state begin
    state: MessageDecodeState,
    message_number: usize,
    payload_size: usize,
    // current message decode data/state end
}

impl Connection {
    pub fn new(tcp_stream: TcpStream, registry: Rc<Registry>, token: Token) -> Self {
        Self {
            tcp_stream,
            registry,
            token,
            message_queue: VecDeque::new(),
            out_buffer: Box::new([0; MAX_PAYLOAD]),
            out_buffer_pos: 0,
            out_buffer_size: 0,
            in_buffer: Box::new([0; MAX_PACKET_SIZE]),
            in_buffer_pos: 0,
            send_interest: false,
            state: MessageDecodeState::WaitingForHeader,
            message_number: 0,
            payload_size: 0,
        }
    }

    /// Returns `true` if the connection should be removed and closed.
    pub fn read(&mut self) -> bool {
        // We can (maybe) read from the connection.
        loop {
            match self
                .tcp_stream
                .read(&mut self.in_buffer[self.in_buffer_pos..])
            {
                Ok(0) => {
                    // Reading 0 bytes means the other side has closed the
                    // connection or is done writing, then so are we.
                    println!("Connection lost.");
                    return true;
                }
                Ok(n) => {
                    self.in_buffer_pos += n;

                    if !self.decode() {
                        println!("Invalid data, closing connection...",);
                        return true;
                    }
                }
                // Would block "errors" are the OS's way of saying that the
                // connection is not actually ready to perform this I/O operation.
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(ref err) if err.kind() == io::ErrorKind::Interrupted => continue,
                // Other errors we'll consider fatal.
                Err(_) => return true,
            }
        }

        return false;
    }

    fn decode(&mut self) -> bool {
        loop {
            if !self.process_states() {
                return false;
            }

            if self.state != MessageDecodeState::PayloadSuccessfulRead {
                break;
            }
        }

        return true;
    }

    fn process_states(&mut self) -> bool {
        loop {
            match self.state {
                MessageDecodeState::WaitingForHeader
                | MessageDecodeState::PayloadSuccessfulRead => {
                    if !self.decode_header() {
                        return false;
                    }
                }
                MessageDecodeState::HeaderSuccessfulRead
                | MessageDecodeState::WaitingForPayload => {
                    if !self.decode_payload() {
                        return false;
                    }
                }
            }

            if self.state == MessageDecodeState::WaitingForHeader
                || self.state == MessageDecodeState::WaitingForPayload
                || self.state == MessageDecodeState::PayloadSuccessfulRead
            {
                return true;
            }
        }
    }

    fn decode_header(&mut self) -> bool {
        // check if the read data length is HEADER_SIZE or more
        if self.in_buffer_pos < HEADER_SIZE {
            self.state = MessageDecodeState::WaitingForHeader;
            return true; // just wait for more data
        }

        // load and check magic
        for i in 0..8 {
            if self.in_buffer[i] != MAGIC[i] {
                return false; // invalid data read
            }
        }

        // load message number
        match self.get_usize(8, 11) {
            Some(message_number) => self.message_number = message_number,
            None => return false,
        }

        // load and check payload size
        match self.get_usize(11, 16) {
            Some(payload_size) => self.payload_size = payload_size,
            None => return false,
        }
        if self.payload_size <= 0 || self.payload_size > MAX_PAYLOAD {
            return false;
        }

        self.state = MessageDecodeState::HeaderSuccessfulRead;
        return true;
    }

    fn decode_payload(&mut self) -> bool {
        // check if the read data length is HEADER_SIZE plus payload size or more
        if self.in_buffer_pos < self.payload_size + HEADER_SIZE {
            self.state = MessageDecodeState::WaitingForPayload;
            return true; // just wait for more data
        }

        // parse json and check message number
        match from_utf8(&self.in_buffer[HEADER_SIZE..self.payload_size + HEADER_SIZE]) {
            Ok(utf8_payload) => match self.message_number {
                0 => ProcessMessage!(PingMessage, utf8_payload, self),
                1 => ProcessMessage!(LoginMessage, utf8_payload, self),
                2 => ProcessMessage!(PublishGlobalChatMessage, utf8_payload, self),
                3 => ProcessMessage!(GlobalChatMessage, utf8_payload, self),
                4 => ProcessMessage!(PublishPrivateChatMessage, utf8_payload, self),
                5 => ProcessMessage!(PrivateChatMessage, utf8_payload, self),
                _ => return false,
            },
            Err(_) => return false,
        }

        // remove packet from buffer
        let mut temp_buffer_pos: usize = 0;
        for i in self.payload_size + HEADER_SIZE..self.in_buffer_pos {
            self.in_buffer[temp_buffer_pos] = self.in_buffer[i];
            temp_buffer_pos += 1;
        }

        self.state = MessageDecodeState::PayloadSuccessfulRead;
        self.in_buffer_pos = temp_buffer_pos;
        return true;
    }

    fn get_usize(&self, buffer_start_pos: usize, buffer_end_pos: usize) -> Option<usize> {
        let mut message_number_string = String::new();
        for i in buffer_start_pos..buffer_end_pos {
            message_number_string.push(self.in_buffer[i] as char);
        }

        match message_number_string.trim_end().parse::<usize>() {
            Ok(read_usize) => Some(read_usize),
            Err(_) => None,
        }
    }

    pub fn send_message(&mut self, message: Message) {
        self.message_queue.push_back(message);
        // return value of send is ignored!!!!
        self.send();
    }

    /// Returns `true` if the connection should be removed and closed.
    pub fn send(&mut self) -> bool {
        loop {
            if self.out_buffer_size == 0 {
                match self.message_queue.pop_front() {
                    Some(message) => {
                        if !self.encode(message.message, message.number) {
                            eprintln!("Error while encode message!");
                            return true;
                        }
                    }
                    None => {
                        if self.send_interest {
                            // disable send interest
                            self.send_interest = false;
                            self.registry
                                .reregister(&mut self.tcp_stream, self.token, Interest::READABLE)
                                .expect("Error while regegister!");
                        }

                        return false;
                    }
                }
            }

            let mut set_send_interest = false;

            // We can (maybe) write to the connection.
            match self
                .tcp_stream
                .write(&self.out_buffer[self.out_buffer_pos..self.out_buffer_size])
            {
                Ok(n) => {
                    if n != self.out_buffer_size - self.out_buffer_pos {
                        self.out_buffer_pos = n;
                        set_send_interest = true;
                    } else {
                        self.out_buffer_size = 0;
                        self.out_buffer_pos = 0;
                        // we dont return because we can try to write more messages
                    }
                }
                // Would block "errors" are the OS's way of saying that the
                // connection is not actually ready to perform this I/O operation.
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => set_send_interest = true,
                // Got interrupted (how rude!), we'll try again.
                Err(ref err) if err.kind() == io::ErrorKind::Interrupted => return self.send(),
                // Other errors we'll consider fatal.
                Err(_) => return true,
            }

            // enable send interest if not already set
            if set_send_interest {
                if !self.send_interest {
                    // enable send interest
                    self.send_interest = true;
                    self.registry
                        .reregister(
                            &mut self.tcp_stream,
                            self.token,
                            Interest::READABLE.add(Interest::WRITABLE),
                        )
                        .expect("Error while regegister!");
                }

                return false;
            }
        }
    }

    pub fn encode(&mut self, message: String, message_number: u32) -> bool {
        let message = message.into_bytes();

        if message.len() > MAX_PAYLOAD {
            return false;
        }

        // write magic
        for i in 0..8 {
            self.out_buffer[i] = MAGIC[i];
        }

        // write message number
        let message_number = message_number.to_string().into_bytes();
        for i in 0..3 {
            if i < message_number.len() {
                self.out_buffer[8 + i] = message_number[i];
            } else {
                self.out_buffer[8 + i] = b' ';
            }
        }

        // write payload size
        let payload_size = message.len().to_string().into_bytes();
        for i in 0..5 {
            if i < payload_size.len() {
                self.out_buffer[11 + i] = payload_size[i];
            } else {
                self.out_buffer[11 + i] = b' ';
            }
        }

        // write payload
        for i in 0..message.len() {
            self.out_buffer[16 + i] = message[i];
        }

        self.out_buffer_size = HEADER_SIZE + message.len();
        self.out_buffer_pos = 0;
        return true;
    }
}
