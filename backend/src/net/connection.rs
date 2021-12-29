use crate::net::msg::msgs::GlobalChatMessage;
use crate::net::msg::msgs::Message;
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
    message_queue: VecDeque<String>,
    out_buffer: Vec<u8>,
    out_buffer_pos: usize,
    in_buffer: [u8; MAX_PACKET_SIZE],
    in_buffer_pos: usize,
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
            out_buffer: Vec::new(),
            out_buffer_pos: 0,
            in_buffer: [0; MAX_PACKET_SIZE],
            in_buffer_pos: 0,
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
                    println!("Connection closed.");
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

        // load payload size
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
                0 => ProcessMessage!(GlobalChatMessage, utf8_payload, self),
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

        match message_number_string.trim_start().parse::<usize>() {
            Ok(read_usize) => Some(read_usize),
            Err(_) => None,
        }
    }

    pub fn send_message(&mut self, message: String) {
        self.message_queue.push_back(message);
        // return value of send is ignored!!!!
        self.send();
    }

    /// Returns `true` if the connection should be removed and closed.
    pub fn send(&mut self) -> bool {
        let mut should_send = true;

        if self.out_buffer.is_empty() {
            match self.message_queue.pop_front() {
                Some(message) => {
                    self.out_buffer = message.into_bytes();
                    self.out_buffer_pos = 0;
                }
                None => {
                    // disable send interest
                    self.registry
                        .reregister(&mut self.tcp_stream, self.token, Interest::READABLE)
                        .expect("Error while regegister!");
                    should_send = false;
                }
            }
        }

        if should_send {
            // We can (maybe) write to the connection.
            match self
                .tcp_stream
                .write(&self.out_buffer[self.out_buffer_pos..])
            {
                // We want to write the entire `DATA` buffer in a single go. If we
                // write less we'll return a short write error (same as
                // `io::Write::write_all` does).
                Ok(n) => {
                    if n == self.out_buffer.len() - self.out_buffer_pos {
                        self.out_buffer.clear();
                        self.out_buffer_pos = 0;
                    } else {
                        self.out_buffer_pos = n;

                        // enable send interest
                        self.registry
                            .reregister(
                                &mut self.tcp_stream,
                                self.token,
                                Interest::READABLE.add(Interest::WRITABLE),
                            )
                            .expect("Error while regegister!");
                    }
                }
                // Would block "errors" are the OS's way of saying that the
                // connection is not actually ready to perform this I/O operation.
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {}
                // Got interrupted (how rude!), we'll try again.
                Err(ref err) if err.kind() == io::ErrorKind::Interrupted => {
                    self.send();
                }
                // Other errors we'll consider fatal.
                Err(_) => return true,
            }
        }

        return false;
    }
}
