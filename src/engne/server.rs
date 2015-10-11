
use mio::*;
use mio::tcp::*;
use mio::util::Slab;
use mio::buf::ByteBuf;
use std::net::SocketAddr;
use std::str::FromStr;
use hlprs::connection::Connection;
use std::collections::BTreeMap;


use std::io::{Result};
use std::str::{from_utf8};
use rustc_serialize::json::{Json};

pub const SERVER: Token = Token(1);


pub fn start_server(host: &str) {

    let addr: SocketAddr = FromStr::from_str(host)
        .ok().expect("Failed to parse host:port string");
    let sock = TcpListener::bind(&addr).ok().expect("Failed to bind address");

    let mut event_loop = EventLoop::new().ok().expect("Failed to create event loop");

    let mut server = Server::new(sock);
    server.register(&mut event_loop).ok().expect("Failed to register server with event loop");

    info!("Even loop starting...");
    event_loop.run(&mut server).ok().expect("Failed to start event loop");
}

pub struct Server {
    sock: TcpListener,
    token: Token,
    conns: Slab<Connection>,
    tree: BTreeMap<String,Vec<Token>>
}

impl Handler for Server {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, event_loop: &mut EventLoop<Server>, token: Token, events: EventSet) {
        debug!("events = {:?}", events);
        assert!(token != Token(0), "[BUG]: Received event for Token(0)");

        if events.is_error() {
            warn!("Error event for {:?}", token);
            self.reset_connection(event_loop, token);
            return;
        }

        if events.is_hup() {
            trace!("Hup event for {:?}", token);
            self.reset_connection(event_loop, token);
            return;
        }

        if events.is_writable() {
            trace!("Write event for {:?}", token);
            assert!(self.token != token, "Received writable event for Server");

            self.find_connection_by_token(token).writable()
                .and_then(|_| self.find_connection_by_token(token).reregister(event_loop))
                .unwrap_or_else(|e| {
                    warn!("Write event failed for {:?}, {:?}", token, e);
                    self.reset_connection(event_loop, token);
                });
        }
        if events.is_readable() {
            trace!("Read event for {:?}", token);
            if self.token == token {
                self.accept(event_loop);
            } else {

                self.readable(event_loop, token)
                    .and_then(|_| self.find_connection_by_token(token).reregister(event_loop))
                    .unwrap_or_else(|e| {
                        warn!("Read event failed for {:?}: {:?}", token, e);
                        self.reset_connection(event_loop, token);
                    });
            }
        }
    }
}

impl Server {
    fn new(sock: TcpListener) -> Server {
        Server {
            sock: sock,
            token: Token(1),
            conns: Slab::new_starting_at(Token(2), 128),
            tree: BTreeMap::new()
        }
    }

    fn register(&mut self, event_loop: &mut EventLoop<Server>) -> Result<()> {
        event_loop.register_opt(
            &self.sock,
            self.token,
            EventSet::readable(),
            PollOpt::edge() | PollOpt::oneshot()
        ).or_else(|e| {
            error!("Failed to register server {:?}, {:?}", self.token, e);
            Err(e)
        })
    }

    fn reregister(&mut self, event_loop: &mut EventLoop<Server>) {
        event_loop.reregister(
            &self.sock,
            self.token,
            EventSet::readable(),
            PollOpt::edge() | PollOpt::oneshot()
        ).unwrap_or_else(|e| {
            error!("Failed to reregister server {:?}, {:?}", self.token, e);
            let server_token = self.token;
            self.reset_connection(event_loop, server_token);
        })
    }

    fn accept(&mut self, event_loop: &mut EventLoop<Server>) {
        debug!("server accepting new socket");

        let sock = match self.sock.accept() {
            Ok(s) => {
                match s {
                    Some(sock) => sock,
                    None => {
                        error!("Failed to accept new socket");
                        self.reregister(event_loop);
                        return;
                    }
                }
            },
            Err(e) => {
                error!("Failed to accept new socket, {:?}", e);
                self.reregister(event_loop);
                return;
            }
        };

        match self.conns.insert_with(|token| {
            debug!("registering {:?} with event loop", token);
            Connection::new(sock, token)
        }) {
            Some(token) => {
                match self.find_connection_by_token(token).register(event_loop) {
                    Ok(_) => {},
                    Err(e) => {
                        error!("Failed to register {:?} connection with event loop, {:?}", token, e);
                        self.conns.remove(token);
                    }
                }
            },
            None => {
                error!("Failed to insert connection into slab");
            }
        };
        self.reregister(event_loop);
    }

    fn parse_message(&mut self, msg: String) {

    }

    fn readable(&mut self, event_loop: &mut EventLoop<Server>, token: Token) -> Result<()> {
        debug!("server conn readable; token={:?}", token);
        let message = try!(self.find_connection_by_token(token).readable());

        if message.remaining() == message.capacity() { // is_empty
            return Ok(());
        }

        // TODO pipeine this whole thing
        let mut bad_tokens = Vec::new();
        let msg = Json::from_str(from_utf8(message.bytes()).unwrap()).unwrap();
        println!("{:?}",msg.as_object().unwrap().get("channel"));
        //self.parse_message(message);



        // Queue up a write for all connected clients.
        for conn in self.conns.iter_mut() {
            // TODO: use references so we don't have to clone
            let conn_send_buf = ByteBuf::from_slice(message.bytes());
            debug!("{:?}",from_utf8(message.bytes()).unwrap().to_string());
            if token != conn.token {
                conn.send_message(conn_send_buf)
                    .and_then(|_| conn.reregister(event_loop))
                    .unwrap_or_else(|e| {
                        error!("Failed to queue message for {:?}: {:?}", conn.token, e);
                        // We have a mutable borrow for the connection, so we cannot remove until the
                        // loop is finished
                        bad_tokens.push(conn.token)
                    });
            }
        }

        for t in bad_tokens {
            self.reset_connection(event_loop, t);
        }
        Ok(())
    }

    fn reset_connection(&mut self, event_loop: &mut EventLoop<Server>, token: Token) {
        if self.token == token {
            event_loop.shutdown();
        } else {
            debug!("reset connection; token={:?}", token);
            self.conns.remove(token);
        }
    }

    /// Find a connection in the slab using the given token.
    fn find_connection_by_token<'a>(&'a mut self, token: Token) -> &'a mut Connection {
        &mut self.conns[token]
    }
}
