extern crate mio;
extern crate bytes;

#[macro_use] extern crate log;
extern crate fern;
extern crate time;

extern crate redis;

use redis::Commands;

use std::io::{Write,Cursor,Read};
use std::rc::Rc;
use std::net::SocketAddr;
use std::str::FromStr;
use std::mem;
use std::sync::mpsc;
use std::io::{Error, ErrorKind};
use bytes::{Buf, Take};

use mio::Evented;
use redis::PubSub;


use std::sync::Arc;



use mio::tcp::{TcpStream,TcpListener};
use mio::util::Slab;
use mio::buf::ByteBuf;
use mio::{TryRead, TryWrite,Token,EventSet,EventLoop,Handler,PollOpt};


const HOST: &'static str = "0.0.0.0:10001";
const LOG_LEVEL: log::LogLevelFilter = log::LogLevelFilter::Debug;
const LOG_FILE_NAME: &'static str = "/Users/sommoyogurt/workspace/rust/punpun/logs/debug.log";
const SERVER: Token = Token(1);


fn init_logger() -> std::io::Result<()> {

    let logger_config = fern::DispatchConfig {
        format: Box::new(|msg: &str, level: &log::LogLevel, _location: &log::LogLocation| {
            format!("[{}] - {} - {}", time::now().strftime("%Y-%m-%d:%H:%M:%S").unwrap(), level, msg)
         }),
        output: vec![fern::OutputConfig::stdout(), fern::OutputConfig::file(LOG_FILE_NAME)],
        level: LOG_LEVEL,
    };
    if let Err(e) = fern::init_global_logger(logger_config, LOG_LEVEL) {
        panic!("Failed to initialize global logger: {}", e);
    }
    Ok(())
}


fn run_subscribe(client: &redis::Client){
    let mut pubsub = client.get_pubsub().ok().expect("Redis Pub/Sub connection error");
    let _ : () = pubsub.subscribe("channel_1").ok().expect("Failed subscribe channel");
    std::thread::spawn(move ||{
            loop {
                let msg = pubsub.get_message().ok().expect("queue message failed");
                let payload : String = msg.get_payload().ok().expect("failed queue payload");
                debug!("channel '{}': {}", msg.get_channel_name(), payload);
            }
        });
}

fn start_server () {

    let client = redis::Client::open("redis://127.0.0.1/").ok().expect("Redis connection error");
    let db = client.get_connection().ok().expect("Redis acquire connection failed");

    let addr: SocketAddr = FromStr::from_str(HOST)
        .ok().expect("Failed to parse host:port string");
    let sock = TcpListener::bind(&addr).ok().expect("Failed to bind address");
    let mut server = Server::new(sock,db);
    let mut ev_loop = EventLoop::new().ok().expect("msg: &str");

    server.register(&mut ev_loop);


    info!("Start server on: {}",HOST);
    ev_loop.run(&mut server);
}

fn main(){
    match init_logger(){
        Ok(r) => debug!("Log initialized"),
        Err(e) => error!("Log not inizialized")
    }
    start_server();
}



struct Connection {
    sock: TcpStream,
    token: Token,
    interest: EventSet,
    queue: Vec<Vec<u8>>
}

impl Connection {
    fn new(sock: TcpStream, token: Token) -> Connection{
        Connection{sock: sock, token: token, interest: EventSet::readable(),queue: Vec::new()}
    }

    fn register(&mut self,ev_loop: &mut EventLoop<Server>) -> std::io::Result<()>{
        self.interest.insert(EventSet::readable());
        ev_loop.register_opt(
            &self.sock,
            self.token,
            self.interest,
            PollOpt::edge() | PollOpt::oneshot()
        ).or_else(|e| {
            error!("Failed to reregister {:?}, {:?}", self.token, e);
            Err(e)
        })
    }

    fn reregister(&mut self, ev_loop: &mut EventLoop<Server>) -> std::io::Result<()>{
        self.interest.insert(EventSet::writable());

        ev_loop.reregister(
            &self.sock,
             self.token,
             self.interest,
             PollOpt::edge() | PollOpt::oneshot()
        ).or_else(|e| {
                 error!("Failed to reregister {:?}, {:?}", self.token, e);
                 Err(e)
        })
    }

    fn read<'a>(&'a mut self, ev_loop: &mut EventLoop<Server>) -> std::io::Result<Vec<u8>>{
        let mut vec = vec![];
        match self.sock.read(&mut vec){
            Ok(0) => {
                debug!("Read 0 bytes from client; buffered={}", 1);
            },
            Ok(n) => {
                debug!("Read {} bytes from client; buffered={}", n, 1);
                self.reregister(ev_loop);
            }
            Err(e) => panic!("got an error trying to read; err={:?}", e)
        };

        Ok(vec)
    }

    fn write(&mut self,ev_loop: &mut EventLoop<Server>) -> std::io::Result<()> {
        try!(self.queue.pop()
            .ok_or(Error::new(ErrorKind::Other, "Could not pop send queue"))
            .and_then(|mut buf| {
                match self.sock.write(&buf) {
                    Ok(0) => {
                        debug!("client flushing buf; WouldBlock");
                        self.queue.push(buf);
                        Ok(())
                    },
                    Ok(n) => {
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

        if self.queue.is_empty() {
            self.interest.remove(EventSet::writable());
        }

        Ok(())
    }

    fn send(&mut self, msg: Vec<u8>) -> std::io::Result<()>{
        self.queue.push(msg);
        self.interest.insert(EventSet::writable());
        Ok(())
    }
}
struct Server{
    sock: TcpListener,
    token: Token,
    socks: Slab<Connection>,
    db: redis::Connection,
}


impl Server {
    fn new(sock: TcpListener, db: redis::Connection) -> Server {
        Server{sock: sock, token: Token(1), socks: Slab::new_starting_at(Token(2),1024),db:db}
    }


    fn accept(&mut self, ev_loop: &mut EventLoop<Server>){
        let sock = match self.sock.accept(){
            Ok(s) => match s {
                    Some(stream) => {
                        debug!("Accept new socket, {:?}",stream.peer_addr());
                        stream
                        },
                    None => {
                        error!("Failed to accept new socket");
                        self.reregister(ev_loop);
                        return;
                    }
                },
            Err(e) => {
                error!("Failed to accept new socket");
                self.reregister(ev_loop);
                return;
            }
        };

        match self.socks.insert_with( |token| {
                debug!("Registering {:?} - {:?} with event loop", token, sock.peer_addr());
                Connection::new(sock, token)
            }){
                Some(token) => {
                    match self.find_connection(token).unwrap().register(ev_loop){
                        Ok(_) => {},
                        Err(e) => {
                            error!("Failed to register {:?} connection with event loop, {:?}", token, e);
                            self.socks.remove(token);
                        }
                    }
                },
                None => error!("Failed to insert connection into slab")
            };
            self.reregister(ev_loop);

    }

    fn register(&mut self, ev_loop: &mut EventLoop<Server>){
        ev_loop.register_opt(
            &self.sock,
            self.token,
            EventSet::readable(),
            PollOpt::edge() | PollOpt::oneshot()
        ).or_else(|e| {
            error!("Failed to register server {:?}, {:?}", self.token, e);
            Err(e)
        });
    }

    fn reregister(&mut self, ev_loop: &mut EventLoop<Server>){
        ev_loop.reregister(
            &self.sock,
            self.token,
            EventSet::readable(),
            PollOpt::edge() | PollOpt::oneshot()
        ).unwrap_or_else(|e| {
            error!("Failed to reregister server {:?}, {:?}", self.token, e);
            let server_token = self.token;
            self.reset_connection(ev_loop, server_token);
        })
    }

    fn write(&mut self, ev_loop: &mut EventLoop<Server>, token: Token){
        debug!("write");
        self.find_connection(token).unwrap().write(ev_loop);
    }

    fn read(&mut self, ev_loop: &mut EventLoop<Server>, token: Token) -> std::io::Result<()> {
         let message = try!(self.find_connection(token).unwrap().read(ev_loop));

         let mut bad_tokens = Vec::new();


         for sock in self.socks.iter_mut(){
             let msg = message.clone();
             sock.send(msg).and_then(|_| sock.reregister(ev_loop))
             .unwrap_or_else(|e| {
                         error!("Failed to queue message for {:?}: {:?}", sock.token, e);
             //             // We have a mutable borrow for the connection, so we cannot remove until the
             //             // loop is finished
                          bad_tokens.push(sock.token)
             //         });
             });
         }
         debug!("Read {:?}",message);

        //
        // if message.remaining() == message.capacity() { // is_empty
        //     return Ok(());
        // }
        //
        // // TODO pipeine this whole thing
        // let mut bad_tokens = Vec::new();
        //
        // for sock in self.socks.iter_mut() {
        //     // TODO: use references so we don't have to clone
        //     let conn_send_buf = ByteBuf::from_slice(message.bytes());
        //     sock.send(conn_send_buf)
        //         .and_then(|_| sock.reregister(ev_loop))
        //         .unwrap_or_else(|e| {
        //             error!("Failed to queue message for {:?}: {:?}", sock.token, e);
        //             // We have a mutable borrow for the connection, so we cannot remove until the
        //             // loop is finished
        //             bad_tokens.push(sock.token)
        //         });
        // }
        //
         for t in bad_tokens {
             self.reset_connection(ev_loop, t);
         }

        Ok(())

    }

    fn reset_connection(&mut self, ev_loop: &mut EventLoop<Server>, token: Token){
        if self.token == token {
            ev_loop.shutdown();
        } else {
            debug!("Reset connection; token={:?}", token);
            self.socks.remove(token);
        }
    }

    fn find_connection<'a>(&'a mut self, token: Token) -> Option<&'a mut Connection>{
        self.socks.get_mut(token)
    }

    fn connection(&mut self, ev_loop: &mut EventLoop<Server>, token: Token, ev: EventSet){
        if ev.is_readable() {
            self.read(ev_loop,token);
        }
        if ev.is_writable() {
            self.write(ev_loop,token);
        }
    }
}


impl Handler for Server {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, ev_loop: &mut EventLoop<Server>, token: Token, ev: EventSet){
        match token {
            SERVER => {
                self.accept(ev_loop);
            },
            _ => {
                self.connection(ev_loop, token, ev);
            }
        }
    }

}






#[cfg(test)]
mod Test{

    use super::{start_server,init_logger,HOST};
    use std::thread;
    use std::net::{TcpStream};
    use std::io::{Read,Write};
    #[test]
    fn start() {
        init_logger();
        thread::spawn(move || {start_server();}).join();
    }

    #[test]
    fn connect(){
        thread::sleep_ms(3000);
        for i in 0..1 {
            let _ = thread::spawn(move || {
                let mut client = TcpStream::connect(HOST).ok().expect("Failed client connection");
                client.write("pipo".as_bytes());
                let mut buf = [0u8;512];
                client.flush();
                client.read(&mut buf);
                client.write("pipo2".as_bytes());
                client.flush();
                client.read(&mut buf);


            });
        }
    }
}
