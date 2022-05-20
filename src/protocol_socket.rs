use std::net::{
    UdpSocket,
    SocketAddr,
};
use std::time::Duration;
use crate::messages::*;
use std::io::ErrorKind;

pub struct ProtocolSocket {
    udp_sock: UdpSocket,
}

// generic error type for ProtocolSocket send errors
#[derive(Debug)]
pub enum SendError {
    SerializationFailed,
    IO(std::io::Error),
    IncompleteSend(usize),
}

// generic error type for ProtocolSocket receive errors
#[derive(Debug)]
pub enum ReceiveError {
    DeserializationFailed,
    IO(std::io::Error),
}

impl ReceiveError {
    pub fn is_fatal(&self) -> bool {
        if let ReceiveError::IO(io_err) = self {
            return match io_err.kind() {
                ErrorKind::WouldBlock => false,
                ErrorKind::TimedOut => false,
                ErrorKind::Interrupted => false,
                _ => true,
            };
        }
        return true;
    }
}

impl ProtocolSocket {
    pub fn bind(bind_addr: &str) -> Result<Self, std::io::Error> {
        let udp_sock = UdpSocket::bind(bind_addr)?;
        Ok(Self {
            udp_sock,
        })
    }

    pub fn get_message(&self) -> Result<(Message, SocketAddr), ReceiveError> {
        let mut buf = [0u8; 65536];

        let (size, source) = match self.udp_sock.recv_from(&mut buf) {
            Ok(x) => x,
            Err(e) => {
                return Err(ReceiveError::IO(e));
            }
        };

        let msg = match Message::deserialize(&buf[0..size]) {
            Ok(msg) => msg,
            Err(()) => {
                return Err(ReceiveError::DeserializationFailed);
            }
        };

        return Ok((msg, source));
    }

    pub fn send_message(&self, msg: &Message, dest: SocketAddr) -> Result<(), SendError>{
        let bytes = match msg.serialize() {
            Ok(data) => data,
            Err(_) => return Err(SendError::SerializationFailed),
        };

        match self.udp_sock.send_to(&bytes[..], dest) {
            Ok(num_bytes) if num_bytes == bytes.len() => {
                return Ok(());
            },
            Err(e) => {
                return Err(SendError::IO(e));
            },
            Ok(n) => {
                return Err(SendError::IncompleteSend(n));
            },
        };
    }
    
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), std::io::Error>  {
        self.udp_sock.set_read_timeout(timeout)
    }
    
    // Get the listening port of the socket.
    // Returns Err if the local address cannot be obtained.
    pub fn get_port(&self) -> Result<u16, ()> {
        match self.udp_sock.local_addr() {
            Ok(addr) => Ok(addr.port()),
            Err(_) => Err(()),
        }
    }
}