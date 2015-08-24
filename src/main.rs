
#[macro_use] extern crate log;
extern crate env_logger;
extern crate mio;
extern crate sha1;
extern crate rustc_serialize;



use std::io;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::str::FromStr;
use std::collections::HashMap;

use mio::*;
use mio::tcp::{TcpListener,TcpStream};
use mio::buf::ByteBuf;
use mio::util::Slab;


use self::rustc_serialize::base64::{ToBase64, STANDARD};

const MAGIC_WS: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const SERVER: Token = Token(0);
const HOST: &'static str = "127.0.0.1:10001";
const HEADER_KEY_VALUE_TRIMMERS: &'static[char] = &[':'];
const CHARS_TO_TRIM: &'static[char] = &[' ', '\r', '\n'];
const UPGRADE: &'static [u8;9] = b"websocket";
const CONNECTION: &'static [u8;7] = b"Upgrade";

static CR: u8 = b'\r';
static LF: u8 = b'\n';
static COLON: u8 = b':';
static SPACE: u8 = b' ';
static SLASH: u8 = b'/';
static HEAD_KEY: &'static [u8;17] = b"Sec-WebSocket-Key";


fn gen_key(key: &String) -> String {
	println!("Socket string:{}",key);
	let mut m = sha1::Sha1::new();
	let mut buf = [0u8;20];

	m.update(key.as_bytes());
	m.update(MAGIC_WS.as_bytes());

	m.output(&mut buf);

	return buf.to_base64(STANDARD);
}

#[derive(Debug)]
#[derive(PartialEq)]
enum ClienState {
    AwaitingHandshake,
    HandshakeResponse,
    Connected
}

#[derive(Debug)]
struct Server {
    sock: TcpListener,
    token: Token,
    socks: Slab<Client>
}

struct Client{
    sock: TcpStream,
    token: Token,
    interest: EventSet,
    state: ClienState,
    header: Header,
    send_queue: Vec<ByteBuf>
}

#[derive(Debug)]
struct Header {
    method: String,
    uri: String,
    http_version: String,
    fields: HashMap<String,String>
}

impl Header {
    fn new() -> Header {
        Header{method: String::new(),uri: String::new(),http_version: String::new(),fields: HashMap::new()}
    }
}
struct Parser;

impl Parser {
    fn new() -> Parser {
        Parser
    }
    fn parser (&self, body: &[u8], h: &mut Header){
        let _h = body.split(|x| *x==10).collect::<Vec<_>>();
        let it = &mut  _h.iter();
        let first_row = self.first_row(it.next().unwrap());
        h.method = first_row.0;
        h.uri = first_row.1;
        h.http_version = first_row.3;
        self.headers(it,h);
    }
    //fn first_row(&self,buf: &[u8]) -> (Method, Uri, Http, HttpVersion) {
    fn first_row(&self,buf: &[u8]) -> (String, String, String, String) {
        println!("In first row: {:?}", std::str::from_utf8(buf));
        let h = buf.split(|x| *x == SPACE).collect::<Vec<_>>();
        println!("Vec : {:?}", h);
        let it = &mut h.iter();
        let method = "Get".to_string();
        it.next();
        let uri = std::str::from_utf8(it.next().unwrap()).unwrap().to_string();
        let http = "http".to_string();
        let http_version = "1.1".to_string();
        /*let method = match it.next() {
            Some(&v) => match v.iter().fold(0, |acc, &x| acc + x) {
                            224 => Method::Get,
                            _ => Method::Missing
                        },
            None => Method::Missing
        };
        let uri: Uri = match it.next() {
            Some(&v) => Uri::Valid(std::str::from_utf8(v).unwrap().to_string()),
            None => Uri::Missing
        };
        let http = Http::Htt;
        let http_version = HttpVersion::Version(1.1);
        */
        println!("Method: {:?} - {:?} - {:?} - {:?}", method, uri, http, http_version);
        (method, uri, http, http_version)
    }

    fn headers(&self,it: &mut std::slice::Iter<&[u8]>, h: &mut Header){
        for i in it {
            let mut s = i.split(|x| *x == SPACE );
            let k = match s.next(){
                Some(mut v) => std::str::from_utf8(v).unwrap().to_string().trim().replace(":",""),
                None => "".to_string()
            };
            let v = match s.next(){
                Some(mut v) => std::str::from_utf8(v).unwrap().to_string().trim().to_string(),
                None => "".to_string()
            };
            println!("{} - {}",k,v);
            h.fields.insert(k,v);
        }
    }
}

impl Client{
    fn new(sock: TcpStream, token: Token) -> Client{
        Client{
            sock: sock,
            token: token,
            interest: EventSet::readable(),
            state: ClienState::AwaitingHandshake,
            header: Header::new(),
            send_queue: Vec::new()
        }
    }

    fn register(&mut self, ev_loop: &mut EventLoop<Server>) -> std::io::Result<()> {
        self.interest.insert(EventSet::readable());

        ev_loop.register_opt(&self.sock, self.token, self.interest
                                , PollOpt::edge() | PollOpt::oneshot()).or_else(|e| {
                                    error!("Failed to reregister {:?}, {:?}", self.token, e);
                                    Err(e)
                                    })
    }

    fn reregister(&mut self, ev_loop: &mut EventLoop<Server>) -> std::io::Result<()> {
        ev_loop.reregister(
            &self.sock,
            self.token,
            self.interest,
            PollOpt::edge() | PollOpt::oneshot())
        .or_else(|e| {
                error!("Failed to reregister {:?}, {:?}", self.token, e);
                Err(e)
        })
    }

    fn readable(&mut self) -> std::io::Result<ByteBuf> {
        let mut buf = ByteBuf::mut_with_capacity(2048);

        loop {
            match self.sock.try_read_buf(&mut buf) {
                Ok(None) => {
                    debug!("CONN : we read 0 bytes");
                    break;
                },
                Ok(Some(n)) => {
                    debug!("CONN : we read {} bytes", n);
                    if n < buf.capacity() {
                        break;
                    }
                },
                Err(e) => {
                    error!("Failed to read buffer for token {:?}, error: {}", self.token, e);
                    return Err(e);
                }
            }
        }
        Ok(buf.flip())
    }

    fn writable(&mut self) -> io::Result<()> {
        try!(self.send_queue.pop()
                    .ok_or(Error::new(ErrorKind::Other, "Could not pop send queue"))
                    .and_then(|mut buf| {
                        match self.sock.try_write_buf(&mut buf) {
                            Ok(None) => {
                                debug!("client flushing buf; WouldBlock");

                                // put message back into the queue so we can try again
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

                /*if self.send_queue.is_empty() {
                    self.interest.remove(EventSet::writable());
                }*/
                Ok(())
    }

    fn writable_handshake(&mut self) -> io::Result<()> {
        let key = match self.header.fields.get("Sec-WebSocket-Key") {
            Some(v) => gen_key(v),
            _ => panic!("No key")
        };

        let buf = format!("HTTP/1.1 101 Switching Protocols\r\n\
        Upgrade: websocket\r\n\
        Connection: Upgrade\r\n\
        Sec-WebSocket-Accept: {}\r\n\
        Sec-WebSocket-Version: 13\r\n\r\n",key);

        match self.sock.try_write_buf(&mut ByteBuf::from_slice(buf.as_bytes())) {
            Ok(None) => {
                debug!("client flushing buf; WouldBlock");
                self.interest.remove(EventSet::writable());
                // put message back into the queue so we can try again
                Ok(())
            },
            Ok(Some(n)) => {
                debug!("CONN : we wrote {} bytes", n);
                self.interest.remove(EventSet::writable());
                Ok(())
            },
            Err(e) => {
                self.interest.remove(EventSet::writable());
                error!("Failed to send buffer for {:?}, error: {}", self.token, e);
                Err(e)
            }
        }
    }

    fn send_message(&mut self, msg: ByteBuf) -> std::io::Result<()> {
        self.send_queue.push(msg);
        self.interest.insert(EventSet::writable());
        Ok(())
    }

    fn is_connected(&mut self) -> bool {
        match self.state {
            ClienState::Connected => true,
            _ => false
        }
    }

    fn parse_header(&mut self) -> std::io::Result<()>{
        let msg = try!(self.readable());
        debug!("{:?}",std::str::from_utf8(msg.bytes()).unwrap().to_string());
        Parser::new().parser(msg.bytes(),&mut self.header);
        Ok(())
    }
}

impl Server {
    fn new(sock: TcpListener) -> Server{
        Server{sock: sock,token: Token(0), socks: Slab::new_starting_at(Token(2),128)}
    }

    fn accept(&mut self,ev_loop: &mut EventLoop<Server>){
        let sock = match self.sock.accept() {
            Ok(s) => match s {
                Some(sock) => sock,
                None => {
                    error!("Failed to accept new socket");
                    self.reregister(ev_loop);
                    return;
                }
            },
            Err(e) => {
                error!("Failed to accept new socket {:?}",e);
                self.reregister(ev_loop);
                return;
            }
        };

        match self.socks.insert_with(
                |token| {
                    debug!("Registring new connection");
                    Client::new(sock, token)
                }
            ){
                Some(token) => {
                    match self.find_connection_by_token(token).register(ev_loop){
                        Ok(_) => info!("New connection registred"),
                        Err(e) => {
                            error!("Failed to register {:?} connection with event loop, {:?}", token, e);
                            self.socks.remove(token);
                        }
                    }
                },
                None => {error!("Failed to insert connection into slab");
            }
        };
        self.reregister(ev_loop);
    }

    fn read_handshake(&mut self, ev_loop: &mut EventLoop<Server>, token: Token) -> std::io::Result<()> {
        debug!("server handshake readable; token={:?}", token);
        let client = self.find_connection_by_token(token);
        client.parse_header();

        client.interest.insert(EventSet::writable());
        client.reregister(ev_loop);

       //debug!("header: {:?}",std::str::from_utf8(msg.bytes()).unwrap().to_string());
       Ok(())
    }

    fn readable(&mut self, ev_loop: &mut EventLoop<Server>, token: Token) -> std::io::Result<()> {
         debug!("server conn readable; token={:?}", token);

        let msg = try!(self.find_connection_by_token(token).readable());

        if msg.remaining() == msg.capacity() {
            return Ok(());
        }

        let mut bad_tokens = Vec::new();

        for client in self.socks.iter_mut(){
            let buf = ByteBuf::from_slice(msg.bytes());
            //let buf = msg;
            client.send_message(buf)
                .and_then(|_| {
                        client.reregister(ev_loop)
                }).unwrap_or_else(|e| {
                    error!("Failed to queue message for {:?}: {:?}", client.token, e);
                    bad_tokens.push(client.token)
                });
        }
        Ok(())
    }

    fn register(&self, ev_loop: &mut EventLoop<Server>) -> std::io::Result<()> {
        ev_loop.register_opt(&self.sock,self.token,EventSet::readable()
            ,PollOpt::edge() | PollOpt::oneshot()).or_else( |e| {
                error!("Failed to register server connection with event loop, {:?}", e);
                Err(e)
            })
    }

    fn reregister(&mut self, ev_loop:&mut EventLoop<Server>){
        ev_loop.reregister(
            &self.sock,
            self.token,
            EventSet::readable(),
            PollOpt::edge() | PollOpt::oneshot())
            .unwrap_or_else(|e| {
                error!("Failed to reregister server {:?}, {:?}", self.token, e);
                let server_token = self.token;
                self.reset_connection(ev_loop, server_token);
            });
    }

    fn find_connection_by_token(&mut self,token: Token) -> &mut Client{
        &mut self.socks[token]
    }

    fn reset_connection(&mut self,ev_loop: &mut EventLoop<Server>, token: Token){
        if self.token == token {
            ev_loop.shutdown();
        }else{
            debug!("reset connection; token={:?}", token);
            self.socks.remove(token);
        }
    }
}

impl Handler for Server {
    type Timeout = u32;
    type Message = String;
    fn ready(&mut self, ev_loop: &mut EventLoop<Server>, token: Token, events: EventSet){
        if(events.is_writable()){
            debug!("Writable");
            let connected = self.find_connection_by_token(token).is_connected();
            if connected {
                trace!("Write event for {:?}", token);
                assert!(self.token != token, "Received writable event for Server");
                self.find_connection_by_token(token).writable()
                                .and_then(|_| self.find_connection_by_token(token).reregister(ev_loop))
                                .unwrap_or_else(|e| {
                                    warn!("Write event failed for {:?}, {:?}", token, e);
                                    self.reset_connection(ev_loop, token);
                                });
            }else{
                trace!("Write event for {:?}", token);
                assert!(self.token != token, "Received writable event for Server");
                self.find_connection_by_token(token).writable_handshake()
                                .and_then(|_| self.find_connection_by_token(token).reregister(ev_loop))
                                .unwrap_or_else(|e| {
                                    warn!("Write event failed for {:?}, {:?}", token, e);
                                    self.reset_connection(ev_loop, token);
                                });
            }
        }

        if events.is_readable() {
            if self.token == token {
                self.accept(ev_loop);
            } else {
                let connected = self.find_connection_by_token(token).is_connected();
                if connected {
                    self.readable(ev_loop,token).and_then(|_|
                        {
                            self.find_connection_by_token(token).reregister(ev_loop)
                        }).unwrap_or_else(|e| {
                            warn!("Read event failed for {:?}: {:?}", token, e);
                            self.reset_connection(ev_loop, token);
                        });
                }else{
                    self.read_handshake(ev_loop,token);
                }
                /*match client.state {
                    ClienState::AwaitingHandshake => {
                        self.handshake(ev_loop,token);
                    },
                    _ => {
                        self.readable(ev_loop,token).and_then(|_|
                            {
                                self.find_connection_by_token(token).reregister(ev_loop)
                            }).unwrap_or_else(|e| {
                                warn!("Read event failed for {:?}: {:?}", token, e);
                                self.reset_connection(ev_loop, token);
                            });
                        }
                    }*/
            }
        }
    }
    fn notify(&mut self, ev_loop: &mut EventLoop<Server>, msg: String){
        println!("Message: {}",msg);
        //ev_loop.shutdown();
    }

    fn timeout(&mut self, ev_loop: &mut EventLoop<Server>, timeout: u32){
        println!("Timeout: {}",timeout);
        ev_loop.shutdown();
    }
}

fn main(){
    env_logger::init().ok().expect("Failed to init log");

    let addr: SocketAddr = FromStr::from_str("127.0.0.1:10001")
        .ok().expect("Failed to parse host:port string");

    let sock = TcpListener::bind(&addr).ok().expect("Failed to bind address");

    let mut ev_loop: EventLoop<Server> = EventLoop::new().ok().expect("Failed to create event loop");


    let mut server = Server::new(sock);

    server.register(&mut ev_loop).ok().expect("Failed to register server with event loop");

    ev_loop.timeout_ms(121,121000).ok().expect("Failed to set timeout to 121 seconds");

    info!("Even loop starting...");
    ev_loop.run(&mut server).ok().expect("Failed to start event loop");
}
