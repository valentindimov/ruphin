use std::net::SocketAddr;
use std::time::{
    Duration,
    Instant,
};
use std::collections::HashMap;
use crate::messages::*;
use crate::protocol_socket::*;

/// Holepuncher's storage of sessions
// TODO complete this!
pub struct SessionStore {
    storage: HashMap<Vec<u8>, SocketAddr>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }
    
    pub fn insert(&mut self, session_id: Vec<u8>, addr: SocketAddr) {
        self.storage.insert(session_id, addr);
    }
    
    pub fn get(&self, session_id: &Vec<u8>) -> Option<SocketAddr> {
        match self.storage.get(session_id) {
            None => None,
            Some(sock_ref) => Some(*sock_ref),
        }
    }
}

/// a holepuncher helps connect servers and clients
pub struct PassiveHolepuncher { 
    /// Underlying socket
    proto_socket: ProtocolSocket,
    /// Storage structure for sessions
    session_store: SessionStore,
}

impl PassiveHolepuncher {
    pub fn new(listen_addr: &str) -> Result<Self, String> {
        // bind a protocol socket
        let proto_socket = match ProtocolSocket::bind(listen_addr) {
            Ok(sock) => sock,
            Err(e) => {
                return Err(format!("Bind error: {:?}", e));
            }
        };
        
        // holepuncher is ready
        return Ok(Self {
            proto_socket,
            session_store: SessionStore::new(),
        });
    }
    
    // Get the listening port of the socket.
    // Returns Err if the local address cannot be obtained.
    pub fn get_port(&self) -> Result<u16, ()> {
        self.proto_socket.get_port()
    }
    
    /// Serve as a holepuncher on the socket.
    /// If time = Some(x), the method returns after a duration of x.
    /// The method also returns upon receiving a LocalInterrupt from localhost, if allow_interrupt is true.
    /// Returns Ok(()) normally, or Err(description) if some error occurred.
    pub fn serve(&mut self, time: Option<Duration>, allow_interrupt: bool) -> Result<(), String> {
        // Represents the current time.
        // Measured before instances of being used if there was a syscall or I/O operation since it was last measured.
        let mut now = Instant::now();
        
        // this is the time when the function should return
        let return_at = match time {
            None => None,
            Some(time) => Some(now + time),
        };
        
        // await messages in a loop
        loop {
            // Re-measure the time since there might've been an I/O operation before that.
            now = Instant::now();
            
            // determine how long the socket should wait
            let socket_time = if let Some(return_at) = return_at {
                // check if we should actually return now
                if now >= return_at {
                    self.proto_socket.set_read_timeout(None).unwrap();
                    return Ok(());
                }
                // otherwise, the socket should wait for return_at - now at most
                Some(return_at - now)
            } else {
                // no return time is specified, so the socket will wait indefinitely.
                None
            };
            
            // set the timeout on the socket
            self.proto_socket.set_read_timeout(socket_time).unwrap();
            
            // await the next message
            match self.proto_socket.get_message() {
                Ok((Message::HelloReq, source)) => {
                    // send the source a HelloResp
                    match self.proto_socket.send_message(&Message::HelloResp, source) {
                        Ok(()) => {},
                        Err(e) => {
                            return Err(format!("Message send error: {:?}", e));
                        }
                    };
                },
                Ok((Message::LocalInterrupt, source)) if allow_interrupt => {
                    // received a local interrupt and interrupts are allowed
                    // check that the source is localhost. If yes, return Ok(None). Otherwise ignore.
                    if source.ip().is_loopback() {
                        self.proto_socket.set_read_timeout(None).unwrap();
                        return Ok(());
                    } else {
                        continue;
                    }
                },
                Ok((Message::Register(contents), source)) => {
                    // add a session to the list of sessions
                    self.session_store.insert(contents.session_id.clone(), source);
                    // respond with a RegisterAck
                    let response = Message::RegisterAck(RegisterAckContents {
                        session_id: contents.session_id,
                    });
                    match self.proto_socket.send_message(&response, source) {
                        Ok(()) => {},
                        Err(e) => {
                            return Err(format!("Message send error: {:?}", e));
                        }
                    };
                },
                Ok((Message::Join(contents), source)) => {
                    if let Some(server) = self.session_store.get(&contents.session_id) {
                        // session found, send the requester the address of the session initiator
                        let response = Message::PeerInfo(PeerInfoContents {
                            peer_addr: server,
                        });
                        match self.proto_socket.send_message(&response, source) {
                            Ok(()) => {},
                            Err(e) => {
                                return Err(format!("Message send error: {:?}", e));
                            }
                        };
                        
                        // also send the session initiator the address of the client
                        let response = Message::PeerInfo(PeerInfoContents {
                            peer_addr: source,
                        });
                        match self.proto_socket.send_message(&response, server) {
                            Ok(()) => {},
                            Err(e) => {
                                return Err(format!("Message send error: {:?}", e));
                            }
                        };
                    } else {
                        // send the source a SessionNotFound error
                        // respond with a RegisterAck
                        let response = Message::SessionNotFound(SessionNotFoundContents {
                            session_id: contents.session_id,
                        });
                        match self.proto_socket.send_message(&response, source) {
                            Ok(()) => {},
                            Err(e) => {
                                return Err(format!("Message send error: {:?}", e));
                            }
                        };
                    }
                },
                Ok(_) => {
                    // another message was received, ignore it
                    continue;
                },
                Err(e) => {
                    if e.is_fatal() {
                        // fatal error, return
                        return Err(format!("Fatal receive error: {:?}", e));
                    } else {
                        // nonfatal error, likely a timeout. Ignore and retry.
                        continue;
                    }
                }
            }
        }
    }
}