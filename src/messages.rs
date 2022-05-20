use std::net::{
    SocketAddr,
    Ipv4Addr,
    Ipv6Addr,
    SocketAddrV4,
    SocketAddrV6,
};

pub const LOCAL_INTERRUPT: u16 = 1;
pub const REGISTER: u16 = 2;
pub const JOIN: u16 = 3;
pub const PEER_INFO: u16 = 4;
pub const DATA: u16 = 5;
pub const REGISTER_ACK: u16 = 6;
pub const SESSION_NOT_FOUND: u16 = 7;
pub const HELLO_REQ: u16 = 8;
pub const HELLO_RESP: u16 = 9;

pub const MAX_DATA_SIZE: usize = 1024;
pub const MAX_SESSION_ID_SIZE: usize = 20;

#[derive(Debug, Clone)]
pub struct RegisterContents {
    pub session_id: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct JoinContents {
    pub session_id: Vec<u8>,
}


#[derive(Debug, Clone)]
pub struct DataContents {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct PeerInfoContents {
    pub peer_addr: SocketAddr,
}

#[derive(Debug, Clone)]
pub struct RegisterAckContents {
    pub session_id: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SessionNotFoundContents {
    pub session_id: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum Message {
    LocalInterrupt,
    Register(RegisterContents),
    Join(JoinContents),
    Data(DataContents),
    PeerInfo(PeerInfoContents),
    RegisterAck(RegisterAckContents),
    SessionNotFound(SessionNotFoundContents),
    HelloReq,
    HelloResp,
}

impl Message {
    fn to_net(x: u16) -> (u8, u8) {
        (u8::try_from(x >> 8).unwrap(), u8::try_from(x & 0xff).unwrap())
    }

    fn from_net(top_byte: u8, bottom_byte: u8) -> u16 {
        (u16::from(top_byte) << 8) | u16::from(bottom_byte)
    }
    
    // internal function for reducing code repetition
    fn serialize_payload_carrier(packet_type: u16, payload: &[u8]) -> Result<Vec<u8>, ()> {
        let payload_len = payload.len();
        let total_len = match u16::try_from(payload_len + 4) {
            Ok(len) => {
                len
            },
            _ => {
                return Err(())
            },
        };
        let mut msg = vec![0u8; usize::from(total_len)];
        // add the header: packet size and type
        let (len_top, len_bot) = Self::to_net(total_len);
        let (type_top, type_bot) = Self::to_net(packet_type);
        
        msg[0] = len_top;
        msg[1] = len_bot;
        msg[2] = type_top;
        msg[3] = type_bot;
        // TODO make this a faster copy method
        for i in 0..payload_len {
            msg[4+i] = payload[i];
        }
        
        return Ok(msg);
    }

    pub fn serialize(&self) -> Result<Vec<u8>, ()> {
        match self {
            Message::LocalInterrupt => {
                // Length = 4, type = 1
                let (type_top, type_bot) = Self::to_net(LOCAL_INTERRUPT);
                return Ok(vec![0u8, 4u8, type_top, type_bot]);
            },
            Message::HelloReq => {
                // Length = 4, type = 1
                let (type_top, type_bot) = Self::to_net(HELLO_REQ);
                return Ok(vec![0u8, 4u8, type_top, type_bot]);
            },
            Message::HelloResp => {
                // Length = 4, type = 1
                let (type_top, type_bot) = Self::to_net(HELLO_RESP);
                return Ok(vec![0u8, 4u8, type_top, type_bot]);
            },
            Message::Register(contents)=> {
                let session_id_len = contents.session_id.len();
                if session_id_len > MAX_SESSION_ID_SIZE {
                    return Err(());
                }
                return Self::serialize_payload_carrier(REGISTER, &contents.session_id);
            },
            Message::RegisterAck(contents)=> {
                let session_id_len = contents.session_id.len();
                if session_id_len > MAX_SESSION_ID_SIZE {
                    return Err(());
                }
                return Self::serialize_payload_carrier(REGISTER_ACK, &contents.session_id);
            },
            Message::Join(contents)=> {
                let session_id_len = contents.session_id.len();
                if session_id_len > MAX_SESSION_ID_SIZE {
                    return Err(());
                }
                return Self::serialize_payload_carrier(JOIN, &contents.session_id);
            },
            Message::SessionNotFound(contents)=> {
                let session_id_len = contents.session_id.len();
                if session_id_len > MAX_SESSION_ID_SIZE {
                    return Err(());
                }
                return Self::serialize_payload_carrier(SESSION_NOT_FOUND, &contents.session_id);
            },
            Message::PeerInfo(contents)=> {
                match contents.peer_addr {
                    SocketAddr::V4(v4_addr) => {
                        // the length here would be 4 B (header) + 1 B (addr type) + 4 B (addr) + 2 B (port) = 11 B
                        let addr_bytes = v4_addr.ip().octets();
                        let (port_top, port_bot) = Self::to_net(v4_addr.port());
                        let (type_top, type_bot) = Self::to_net(PEER_INFO);
                        return Ok(
                            vec![
                                0u8, 11u8, type_top, type_bot,
                                4u8, addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3],
                                port_top, port_bot,
                        ]);
                    },
                    SocketAddr::V6(v6_addr) => {
                        // the length here would be 4 B (header) + 1 B (addr type) + 16 B (addr) + 2 B (port) = 23 B
                        let addr_bytes = v6_addr.ip().octets();
                        let (port_top, port_bot) = Self::to_net(v6_addr.port());
                        let (type_top, type_bot) = Self::to_net(PEER_INFO);
                        return Ok(
                            vec![
                                0u8, 23u8, type_top, type_bot,
                                6u8,
                                addr_bytes[0], addr_bytes[1], addr_bytes[2], addr_bytes[3],
                                addr_bytes[4], addr_bytes[5], addr_bytes[6], addr_bytes[7],
                                addr_bytes[8], addr_bytes[9], addr_bytes[10], addr_bytes[11],
                                addr_bytes[12], addr_bytes[13], addr_bytes[14], addr_bytes[15],
                                port_top, port_bot,
                            ]);
                    },
                }
            },
            Message::Data(contents)=> {
                let data_len = contents.data.len();
                if data_len > MAX_DATA_SIZE {
                    return Err(());
                }
                return Self::serialize_payload_carrier(DATA, &contents.data);
            },
        }
    }

    pub fn deserialize(from: &[u8]) -> Result<Message, ()> {
        // measure and check the size of the package
        let length = from.len();
        if length < 4 {
            // error: not enough bytes for the header
            return Err(());
        }
        
        // header consists of length and message type
        let len_top = from[0];
        let len_bot = from[1];
        let type_top = from[2];
        let type_bot = from[3];
        
        // parse the type
        let msg_type = Self::from_net(type_top, type_bot);

        // check that the stated length matches the actual message length
        // since we're working with datagrams, it should match exactly
        if length != usize::from(Self::from_net(len_top, len_bot)) {
            return Err(());
        }

        match msg_type {
            LOCAL_INTERRUPT => {
                if length == 4 {
                    return Ok(Message::LocalInterrupt);
                } else {
                    return Err(());
                }
            },
            HELLO_REQ => {
                if length == 4 {
                    return Ok(Message::HelloReq);
                } else {
                    return Err(());
                }
            },
            HELLO_RESP => {
                if length == 4 {
                    return Ok(Message::HelloResp);
                } else {
                    return Err(());
                }
            },
            REGISTER => {
                let session_id_len = length - 4;
                if session_id_len > MAX_SESSION_ID_SIZE {
                    // session ID too big
                    return Err(())
                }
                
                let mut session_id = vec![0u8; session_id_len];
                // TODO more efficient data copying
                for i in 0..session_id_len {
                    session_id[i] = from[4+i]
                }
                return Ok(Message::Register(RegisterContents {
                    session_id
                }));
            },
            REGISTER_ACK => {
                let session_id_len = length - 4;
                if session_id_len > MAX_SESSION_ID_SIZE {
                    // session ID too big
                    return Err(())
                }
                
                let mut session_id = vec![0u8; session_id_len];
                // TODO more efficient data copying
                for i in 0..session_id_len {
                    session_id[i] = from[4+i]
                }
                return Ok(Message::RegisterAck(RegisterAckContents {
                    session_id
                }));
            },
            JOIN => {
                let session_id_len = length - 4;
                if session_id_len > MAX_SESSION_ID_SIZE {
                    // session ID too big
                    return Err(())
                }
                
                let mut session_id = vec![0u8; session_id_len];
                // TODO more efficient data copying
                for i in 0..session_id_len {
                    session_id[i] = from[4+i]
                }
                return Ok(Message::Join(JoinContents {
                    session_id
                }));
            },
            SESSION_NOT_FOUND => {
                let session_id_len = length - 4;
                if session_id_len > MAX_SESSION_ID_SIZE {
                    // session ID too big
                    return Err(())
                }
                
                let mut session_id = vec![0u8; session_id_len];
                // TODO more efficient data copying
                for i in 0..session_id_len {
                    session_id[i] = from[4+i]
                }
                return Ok(Message::SessionNotFound(SessionNotFoundContents {
                    session_id
                }));
            },
            PEER_INFO => {
                if length == 11 && from[4] == 4 {
                    // IPv4 address
                    let port = Self::from_net(from[9], from[10]);
                    let addr = Ipv4Addr::from([from[5], from[6], from[7], from[8]]);
                    let peer_addr = SocketAddr::V4(SocketAddrV4::new(addr, port));
                    return Ok(Message::PeerInfo(PeerInfoContents {
                        peer_addr,
                    }));
                } else if length == 23 && from[4] == 6 {
                    // IPv6 address
                    let port = Self::from_net(from[21], from[22]);
                    let addr = Ipv6Addr::from([
                        from[5], from[6], from[7], from[8],
                        from[9], from[10], from[11], from[12],
                        from[13], from[14], from[15], from[16],
                        from[17], from[18], from[19], from[20],
                    ]);
                    let peer_addr = SocketAddr::V6(SocketAddrV6::new(addr, port, 0, 0));
                    return Ok(Message::PeerInfo(PeerInfoContents {
                        peer_addr,
                    }));
                } else {
                    return Err(());
                }
            },
            DATA => {
                let data_len = length - 4;
                if data_len > MAX_DATA_SIZE {
                    // datagram too big
                    return Err(())
                }
                
                let mut data = vec![0u8; data_len];
                // TODO more efficient data copying
                for i in 0..data_len {
                    data[i] = from[4+i]
                }
                return Ok(Message::Data(DataContents {
                    data
                }));
            },
            _ => {
                return Err(());
            },
        }
    }
}