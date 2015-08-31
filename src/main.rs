
#[macro_use]
extern crate log;
extern crate fern;
extern crate time;

extern crate mio;
extern crate rmp;
extern crate rmp_serialize;
extern crate rustc_serialize;

use log::*;

use std::io;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::str::FromStr;

use mio::*;
use mio::buf::ByteBuf;
use mio::tcp::*;
use mio::util::Slab;

use rustc_serialize::{Encodable,Decodable};
use rmp_serialize::{Encoder,Decoder};


const HOST: &'static str = "0.0.0.0:10001";

#[derive(RustcEncodable, RustcDecodable, PartialEq, Debug)]
struct Message {
    ch: String,
    msg: String,
    time: String
}




fn start_server() {

    let logger_config = fern::DispatchConfig {
        format: Box::new(|msg: &str, level: &log::LogLevel, _location: &log::LogLocation| {
            format!("[{}] - {} - {}", time::now().strftime("%Y-%m-%d:%H:%M:%S").unwrap(), level, msg)
         }),
        output: vec![fern::OutputConfig::stdout(), fern::OutputConfig::file("output.log")],
        level: log::LogLevelFilter::Debug,
    };
    if let Err(e) = fern::init_global_logger(logger_config, log::LogLevelFilter::Debug) {
        panic!("Failed to initialize global logger: {}", e);
    }
    //env_logger::init().ok().expect("Failed to init logger");

    let addr: SocketAddr = FromStr::from_str(HOST)
        .ok().expect("Failed to parse host:port string");
    let sock = TcpListener::bind(&addr).ok().expect("Failed to bind address");

    let mut event_loop = EventLoop::new().ok().expect("Failed to create event loop");

    let mut server = Server::new(sock);
    server.register(&mut event_loop).ok().expect("Failed to register server with event loop");

    info!("Even loop starting...");
    event_loop.run(&mut server).ok().expect("Failed to start event loop");
}

fn main() {
    start_server()

}


struct Server {
    sock: TcpListener,
    token: Token,
    conns: Slab<Connection>,
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
            conns: Slab::new_starting_at(Token(2), 128)
        }
    }

    fn register(&mut self, event_loop: &mut EventLoop<Server>) -> io::Result<()> {
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

    fn readable(&mut self, event_loop: &mut EventLoop<Server>, token: Token) -> io::Result<()> {
        debug!("server conn readable; token={:?}", token);
        let message = try!(self.find_connection_by_token(token).readable());

        if message.remaining() == message.capacity() { // is_empty
            return Ok(());
        }

        // TODO pipeine this whole thing
        let mut bad_tokens = Vec::new();

        // Queue up a write for all connected clients.
        for conn in self.conns.iter_mut() {
            // TODO: use references so we don't have to clone
            let conn_send_buf = ByteBuf::from_slice(message.bytes());
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

struct Connection {
    sock: TcpStream,
    token: Token,
    interest: EventSet,
    send_queue: Vec<ByteBuf>,
}

impl Connection {
    fn new(sock: TcpStream, token: Token) -> Connection {
        Connection {
            sock: sock,
            token: token,
            interest: EventSet::hup(),
            send_queue: Vec::new(),
        }
    }

    fn readable(&mut self) -> io::Result<ByteBuf> {
        let mut recv_buf = ByteBuf::mut_with_capacity(2048);
        loop {
            match self.sock.try_read_buf(&mut recv_buf) {
                Ok(None) => {
                    debug!("CONN : we read 0 bytes");
                    break;
                },
                Ok(Some(n)) => {
                    debug!("CONN : we read {} bytes", n);
                    if n < recv_buf.capacity() {
                        break;
                    }
                },
                Err(e) => {
                    error!("Failed to read buffer for token {:?}, error: {}", self.token, e);
                    return Err(e);
                }
            }
        }
        Ok(recv_buf.flip())
    }

    fn writable(&mut self) -> io::Result<()> {

        try!(self.send_queue.pop()
            .ok_or(Error::new(ErrorKind::Other, "Could not pop send queue"))
            .and_then(|mut buf| {
                match self.sock.try_write_buf(&mut buf) {
                    Ok(None) => {
                        debug!("client flushing buf; WouldBlock");
                        self.send_queue.push(buf);
                        Ok(())
                    },
                    Ok(Some(n)) => {
                        debug!("CONN : we wrote {} bytes", n);
                        Ok(())
                    },
                    Err(e) => {
                        error!("Failed to send buffer for {:?}, error: {}", self.token, e);
                        Err(e)
                    }
                }
            })
        );

        if self.send_queue.is_empty() {
            self.interest.remove(EventSet::writable());
        }

        Ok(())
    }

    fn send_message(&mut self, message: ByteBuf) -> io::Result<()> {
        self.send_queue.push(message);
        self.interest.insert(EventSet::writable());
        Ok(())
    }
    fn register(&mut self, event_loop: &mut EventLoop<Server>) -> io::Result<()> {
        self.interest.insert(EventSet::readable());

        event_loop.register_opt(
            &self.sock,
            self.token,
            self.interest,
            PollOpt::edge() | PollOpt::oneshot()
        ).or_else(|e| {
            error!("Failed to reregister {:?}, {:?}", self.token, e);
            Err(e)
        })
    }

    fn reregister(&mut self, event_loop: &mut EventLoop<Server>) -> io::Result<()> {
        event_loop.reregister(
            &self.sock,
            self.token,
            self.interest,
            PollOpt::edge() | PollOpt::oneshot()
        ).or_else(|e| {
            error!("Failed to reregister {:?}, {:?}", self.token, e);
            Err(e)
        })
    }
}

#[cfg(test)]
mod Test {
    use rustc_serialize::{Encodable,Decodable};
    use rmp_serialize::{Encoder,Decoder};
    use super::{start_server,HOST, Message};
    use std::io::prelude::*;
    use std::net::TcpStream;
    use std::thread;
    use std::str;

    #[test]
    fn start() {
        thread::spawn(move || {start_server();}).join();
    }

    #[test]
    fn client(){
        thread::sleep_ms(3000);
        let smsg = Message{ch:"po".to_string(),msg:"po".to_string(),time:"po".to_string()};
        let mut buf = "pipo".as_bytes();
        //smsg.encode(&mut Encoder::new(&mut &mut buf[..]));
        for i in 0..10 {

            let _ = thread::spawn(move|| {

                let mut stream = TcpStream::connect(HOST).unwrap();

                for x in 0..100 {
                    stream.write(&buf);

                    let mut r = [0u8; 256];
                    match stream.read(&mut r) {
                        Ok(0) => {
                            debug!("thread {}: 0 bytes read", i+x);
                        },
                        Ok(n) => {
                            debug!("thread {}: {} bytes read", i+x, n);

                            let s = str::from_utf8(&r[..]).unwrap();
                            debug!("thread {} read = {}", i+x, s);
                        },
                        Err(e) => {
                            debug!("thread {}: {}", i+x, e);
                        }
                    }
                }
            });
        }

    }

    fn write(stream: &mut TcpStream) {
        stream.write("pipopo".as_bytes());
    }

}
