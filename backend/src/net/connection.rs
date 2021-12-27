use crate::net::msg::message_factory::MessageFactory;
use mio::net::TcpStream;

pub struct Connection {
    pub connection: TcpStream,
    pub message_factory: MessageFactory,
}

impl Connection {
    pub fn new(connection: TcpStream) -> Self {
        Self {
            connection,
            message_factory: MessageFactory::new(),
        }
    }
}
