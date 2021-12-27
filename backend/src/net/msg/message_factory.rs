use crate::net::msg::msgs::GlobalChatMessage;
use std::str::from_utf8;

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

#[derive(PartialEq, Eq)]
enum MessageFactoryState {
    WaitingForHeader,
    HeaderSuccessfulRead,
    WaitingForPayload,
    PayloadSuccessfulRead,
}

pub struct MessageFactory {
    pub buffer: [u8; MAX_PACKET_SIZE],
    pub buffer_pos: usize,
    // current message decode data/state begin
    state: MessageFactoryState,
    message_number: usize,
    payload_size: usize,
    // current message decode data/state end
}

impl MessageFactory {
    pub fn new() -> Self {
        Self {
            buffer: [0; MAX_PACKET_SIZE],
            buffer_pos: 0,
            state: MessageFactoryState::WaitingForHeader,
            message_number: 0,
            payload_size: 0,
        }
    }

    pub fn decode(&mut self) -> bool {
        loop {
            if !self.process_states() {
                return false;
            }

            if self.state != MessageFactoryState::PayloadSuccessfulRead {
                break;
            }
        }

        return true;
    }

    fn process_states(&mut self) -> bool {
        loop {
            match self.state {
                MessageFactoryState::WaitingForHeader
                | MessageFactoryState::PayloadSuccessfulRead => {
                    if !self.decode_header() {
                        return false;
                    }
                }
                MessageFactoryState::HeaderSuccessfulRead
                | MessageFactoryState::WaitingForPayload => {
                    if !self.decode_payload() {
                        return false;
                    }
                }
            }

            if self.state == MessageFactoryState::WaitingForHeader
                || self.state == MessageFactoryState::WaitingForPayload
                || self.state == MessageFactoryState::PayloadSuccessfulRead
            {
                return true;
            }
        }
    }

    fn decode_header(&mut self) -> bool {
        // check if the read data length is HEADER_SIZE or more
        if self.buffer_pos < HEADER_SIZE {
            self.state = MessageFactoryState::WaitingForHeader;
            return true; // just wait for more data
        }

        // load and check magic
        for i in 0..8 {
            if self.buffer[i] != MAGIC[i] {
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

        self.state = MessageFactoryState::HeaderSuccessfulRead;
        return true;
    }

    fn decode_payload(&mut self) -> bool {
        // check if the read data length is HEADER_SIZE plus payload size or more
        if self.buffer_pos < self.payload_size + HEADER_SIZE {
            self.state = MessageFactoryState::WaitingForPayload;
            return true; // just wait for more data
        }

        // parse json
        match from_utf8(&self.buffer[HEADER_SIZE..self.payload_size + HEADER_SIZE]) {
            Ok(utf8_payload) => match serde_json::from_str::<GlobalChatMessage>(utf8_payload) {
                Ok(global_chat_message) => {
                    println!("{}", global_chat_message.message);
                }
                Err(_) => return false,
            },
            Err(_) => return false,
        }

        // remove packet from buffer
        let mut temp_buffer_pos: usize = 0;
        for i in self.payload_size + HEADER_SIZE..self.buffer_pos {
            self.buffer[temp_buffer_pos] = self.buffer[i];
            temp_buffer_pos += 1;
        }

        self.state = MessageFactoryState::PayloadSuccessfulRead;
        self.buffer_pos = temp_buffer_pos;
        return true;
    }

    fn get_usize(&self, buffer_start_pos: usize, buffer_end_pos: usize) -> Option<usize> {
        let mut message_number_string = String::new();
        for i in buffer_start_pos..buffer_end_pos {
            message_number_string.push(self.buffer[i] as char);
        }

        match message_number_string.trim_start().parse::<usize>() {
            Ok(read_usize) => Some(read_usize),
            Err(_) => None,
        }
    }
}
