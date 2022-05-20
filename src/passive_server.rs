use std::net::SocketAddr;
use std::time::{
    Duration,
    Instant,
};
use crate::messages::*;
use crate::protocol_socket::*;

/// a server maintains and serves on a session
pub struct PassiveServer { 
    /// Underlying socket
    proto_socket: ProtocolSocket,
    /// Address of the holepuncher the session is registered with
    holepuncher: SocketAddr,
    /// ID of the session
    session_id: Vec<u8>,
    /// Keepalive interval. Default is 10 seconds.
    keepalive_interval: Duration,
    /// Time after which the server should send a keepalive to the holepuncher.
    next_keepalive_at: Instant,
}

impl PassiveServer {
    pub fn new(holepuncher: SocketAddr, session_id: Vec<u8>)
        -> Result<Self, String> {
        // bind a protocol socket to 0.0.0.0:0
        let sock = match ProtocolSocket::bind("0.0.0.0:0") {
            Ok(sock) => sock,
            Err(e) => {
                return Err(format!("Socket bind error: {:?}", e));
            }
        };
        
        // Timeout behaviour:
        // Up to 10 seconds for the session
        // individual message timeout = 500 ms
        // minimal inter-message time = 400 ms
        // 3 Hello retries
        let total_timeout = Duration::from_secs(10);
        let indiv_timeout = Duration::from_millis(500);
        let inter_message_time = Duration::from_millis(400);
        
        // deadline after which the attempt to create a server is considered failed
        let end_time = Instant::now() + total_timeout;
        // Set the protocol socket's message timeout (will be undone after the function returns)
        sock.set_read_timeout(Some(indiv_timeout)).unwrap();
        
        // Now we will send a Register to the holepuncher, and expect a RegisterAck back.
        let request = Message::Register(RegisterContents {
            session_id: session_id.clone(),
        });
        
        // send the request initially
        match sock.send_message(&request, holepuncher) {
            Ok(()) => {},
            Err(e) => {
                return Err(format!("Message send error: {:?}", e));
            }
        };
        // earliest time after which the next retry will be sent
        let mut next_retry_at = Instant::now() + inter_message_time;
        
        // enter a retry loop
        while Instant::now() < end_time {
            // if we're past the next_retry_at deadline, retry sending the Register and reset the next_retry_at deadline
            if Instant::now() > next_retry_at {
                match sock.send_message(&request, holepuncher) {
                    Ok(()) => {
                        // reset the next_retry_at deadline
                        next_retry_at = Instant::now() + inter_message_time
                    },
                    Err(e) => {
                        return Err(format!("Message send error: {:?}", e));
                    }
                };
            }
            
            // Wait for a response. This will either succeed, timeout, or fail fatally.
            let (ack, source) = match sock.get_message() {
                Ok((ack, source)) => (ack, source),
                Err(e) => {
                    if e.is_fatal() {
                        // fatal error, return
                        return Err(format!("Fatal receive error: {:?}", e));
                    } else {
                        // nonfatal error, ignore and retry
                        continue;
                    }
                },
            };
            
            // We got a message. What is it?
            if let Message::RegisterAck(RegisterAckContents {
                session_id: returned_session_id
            }) = ack {
                // it's a session register acknowledgement
                if source != holepuncher {
                    // message is not from the holepuncher, ignore it
                    continue;
                }
                if returned_session_id != session_id {
                    // message has a wrong session ID, ignore it
                    continue;
                }
                // our session was registered successfully!
                // remove the timeout on the socket
                sock.set_read_timeout(None).unwrap();
                // construct an Endpoint and return it
                return Ok(Self {
                    proto_socket: sock,
                    holepuncher,
                    session_id,
                    keepalive_interval: Duration::from_secs(10),
                    next_keepalive_at: Instant::now() + Duration::from_secs(10),
                });
            } else {
                // some other message arrived, ignore it and retry
                continue;
            }
        }
        
        // timeout, could not register session
        return Err(format!("Timed out trying to register the session."));
    }
    
    // Get the listening port of the socket.
    // Returns Err if the local address cannot be obtained.
    pub fn get_port(&self) -> Result<u16, ()> {
        self.proto_socket.get_port()
    }
    
    // Sends a datagram through the protocol socket to the given target
    pub fn send_datagram(&mut self, to: SocketAddr, data: Vec<u8>) -> Result<(), String> {
        let msg = Message::Data(DataContents {
            data,
        });
        
        match self.proto_socket.send_message(&msg, to) {
            Ok(()) => {
                return Ok(());
            },
            Err(e) => {
                return Err(format!("Message send error: {:?}", e));
            }
        }
    }
    
    /// Serve messages on the socket until you get a datagram from someone.
    /// This method should be called regularly to ensure keepalives are sent, connection requests answered, etc.
    /// If no data is received after a specified timeout, it returns Ok(None).
    /// If a timeout of None is specified, this function will not return until it has data.
    /// An exception to this is: If allow_interrupt is true, the function will return if it receives a LocalInterrupt message from localhost, again with Ok(None).
    pub fn wait_for_data(&mut self, timeout: Option<Duration>, allow_interrupt: bool) -> Result<Option<(SocketAddr, Vec<u8>)>, String> {
        // Represents the current time.
        // Measured before instances of being used if there was a syscall or I/O operation since it was last measured.
        let mut now = Instant::now();
        
        // this is the time when the function should return
        let return_at = match timeout {
            None => None,
            Some(timeout) => Some(now + timeout),
        };
        
        // await messages in a loop
        loop {
            // Re-measure the time since there might've been an I/O operation before that.
            now = Instant::now();
            
            // Is it time to send a keepalive?
            if now > self.next_keepalive_at {
                // send a keepalive (Register for my session) to the holepuncher
                let msg = Message::Register(RegisterContents {
                    session_id: self.session_id.clone()
                });
                let addr = self.holepuncher;
                
                // TODO we can track the time since the last RegisterAck to see if the holepuncher is still online?
                match self.proto_socket.send_message(&msg, addr) {
                    Ok(()) => {},
                    Err(e) => {
                        return Err(format!("Message send error: {:?}", e));
                    }
                };
                // We did an I/O operation, so re-measure the current time.
                now = Instant::now();
                
                // schedule the next keepalive
                self.next_keepalive_at = now + self.keepalive_interval;
            }
            
            // Is it time to return?
            if let Some(return_at) = return_at {
                if now > return_at {
                    self.proto_socket.set_read_timeout(None).unwrap();
                    return Ok(None);
                }
            }
            
            // determine the next wakeup time
            let next_wakeup = if let Some(return_at) = return_at {
                if return_at > self.next_keepalive_at {
                    // Have to first do a keepalive
                    self.next_keepalive_at
                } else {
                    // Return before it's time for the keepalive
                    return_at
                }
            } else {
                // no return time; wake up when it's time for the next keepalive
                self.next_keepalive_at 
            };
            
            // determine how much time we give the socket to wait for messages
            let socket_time = {
                if next_wakeup <= now {
                    // no time, return to beginning of loop
                    continue;
                } else {
                    // roughly until next_wakeup
                    next_wakeup - now
                }
            };
            
            // set the timeout on the socket
            self.proto_socket.set_read_timeout(Some(socket_time)).unwrap();
            
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
                Ok((Message::PeerInfo(contents), source)) => {
                    // got a PeerInfo packet 
                    // ignore it unless it's coming from the holepuncher
                    if source == self.holepuncher {
                        // send a HelloReq to the peer, once.
                        match self.proto_socket.send_message(&Message::HelloReq, contents.peer_addr) {
                            Ok(()) => {},
                            Err(e) => {
                                return Err(format!("Message send error: {:?}", e));
                            }
                        };
                    }
                },
                Ok((Message::Data(contents), source)) => {
                    // got some data, return it
                    // remove the timeout on the socket
                    // TODO check data source?
                    self.proto_socket.set_read_timeout(None).unwrap();
                    return Ok(Some((source, contents.data)));
                },
                Ok((Message::LocalInterrupt, source)) if allow_interrupt => {
                    // received a local interrupt and interrupts are allowed
                    // check that the source is localhost. If yes, return Ok(None). Otherwise ignore.
                    if source.ip().is_loopback() {
                        self.proto_socket.set_read_timeout(None).unwrap();
                        return Ok(None);
                    } else {
                        continue;
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