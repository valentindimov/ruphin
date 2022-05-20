# ruphin - Rust-based UDP cross-NAT networking
ruphin is a simple UDP-based hole-punching tool, currently in active development. It offers a way to allow peers behind a NAT to directly send UDP messages to each other, with minimal involvement of a server in order to establish the connection.

In order for peers to send data to each other using ruphin, both first perform a handshake with a holepuncher server. After this handshake, the two peers can directly send data to each other, with no further involvement from the holepuncher needed. It works roughly as follows:

- One of the peers is called the "server" peer. The server registers a session with the holepuncher. The session is identified with a session ID.
- The other peer, the "client" peer, requests that session from the holepuncher by sending it the same session ID.
- The holepuncher sends both peers their respective IP addresses and ports. The two clients then simultaneously try to send messages to each other. This has the effect of "manipulating" most NATs and firewalls into assuming that there's an ongoing connection between the two peers.
- The two peers can now send data directly to each other. This data transfer has the same semantics as UDP: connectionless, unreliable datagram transport.

Note: The holepuncher retains the session information after the client has connected, and the server peer maintains its session registration. This means that it's possible for multiple clients to "punch holes" and send data to the server, hence the "server" and "client" semantics.

To use ruphin, add the following line to your Cargo.toml:

    [dependencies]
    ruphin = { TODO }
    
ruphin is implemented synchronously and does not have any dependencies other than the Rust standard library. It should build and function reasonably well on any platform where the Rust standard library is available. It has been tested on Ubuntu 20.04 (x64) and Windows 10 (x64).

## Overview of modules
Currently, the library offers three passive modules:

- `PassiveClient`, an implementation of the client peer;
- `PassiveServer`, an implementation of the server peer;
- `PassiveHolepuncher`, an implementation of the holepuncher;

The modules are passive in the sense that they create objects, where a method needs to be periodically invoked so that the module can respond to protocol messages, send keepalives, etc. They are useful for scenarios where dedicating a separate thread to these tasks is impossible or undesirable.

Active modules, where a separate thread manages these tasks, will be released soon.

## Reference implementations
See TODO for reference implementations of a holepuncher, server and client peer.

## Protocol documentation
TODO 

## Security
In its current form, ruphin does not implement or offer security-related features such as encryption of information or authentication of peers. I have not yet decided what security features, if any, I should implement, and how.