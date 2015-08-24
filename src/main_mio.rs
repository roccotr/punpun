extern crate punpun;
extern crate mio;
extern crate http_muncher;
extern crate sha1;
extern crate rustc_serialize;


use std::collections::HashMap;
use std::io::{Read,Write};
//use std::io::prelude::*;
use std::thread;
use mio::{EventLoop,EventSet,Handler,Token,PollOpt};
use mio::tcp::{TcpListener, TcpStream};
use punpun::hlprs::dataframe::*;

//use http_muncher::{Parser,ParserHandler};

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


#[derive(Debug)]
pub enum Method {
        Get,
        Post,
        Put,
        Delete,
        Missing
}

#[derive(Debug)]
pub enum Uri {
        Valid(String),
        Missing,
}

#[derive(Debug)]
pub enum Http {
        Http,
        Https,
        Missing
}

#[derive(Debug)]
pub enum HttpVersion {
        Version(f32),
        Missing
}

#[derive(Debug)]
pub enum Upgrade {
        Websocket,
        Missing
}

#[derive(Debug)]
pub enum Connection {
        Websocket,
        Missing
}

#[derive(Debug)]
pub enum SocketKey {
        Key(String),
        Missing
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

pub fn gen_key(key: &String) -> String {
	println!("Socket string:{}",key);
	let mut m = sha1::Sha1::new();
	let mut buf = [0u8;20];

	m.update(key.as_bytes());
	m.update(MAGIC_WS.as_bytes());

	m.output(&mut buf);

	return buf.to_base64(STANDARD);
}

#[derive(Debug)]
struct MyHandler {
    i: usize,
    socks: HashMap<Token,MyHandlerClient>,
    server: TcpListener,
}

#[derive(Debug)]
#[derive(PartialEq)]
enum ClienState {
    AwaitingHandshake,
    HandshakeResponse,
    Connected
}

#[derive(Debug)]
struct MyHandlerClient{
    sock: TcpStream,
    header: Header,
    interest: EventSet,
    state: ClienState,
}

impl MyHandlerClient{
    fn new(sock: TcpStream) -> MyHandlerClient {
        MyHandlerClient{sock:sock, header: Header::new(),interest:EventSet::readable(),state: ClienState::AwaitingHandshake}
    }
}

impl MyHandler{
    fn new(server: TcpListener) -> MyHandler{
        MyHandler{i:0,socks:HashMap::new(),server:server}
    }
}

impl Handler for MyHandler{
    type Timeout = u32;
    type Message = String;
    fn ready(&mut self, ev_loop: &mut EventLoop<MyHandler>, token: Token, ev_set: EventSet){
        if ev_set.is_readable() {
        match token {
            SERVER => {
                match self.server.accept() {
                    Ok(stream) => {
                        match stream{
                            Some(sock) => {
                                println!("{:?} - {:?} - {:?}",sock,sock.peer_addr(),sock.local_addr());
                                self.i +=1;
                                let tok = Token(self.i);
                                self.socks.insert(Token(self.i),MyHandlerClient::new(sock));
                                let res = ev_loop.register_opt(&self.socks[&tok].sock,tok,EventSet::readable()
                                        ,PollOpt::edge() | PollOpt::oneshot());
                                println!("{:?}",res);
                            },
                            _ => panic!("Stream problema")
                        }
                    },
                    Err(_) => panic!("Acquire stream error")
                };
            },
            client => {
                match self.socks.get_mut(&client) {
                    Some(c) => {
                                    match c.state {
                                        ClienState::Connected => {
                                            match read_dataframe(&mut c.sock,true){
                                                Ok(v) => {
                                                    let s = String::from_utf8(v.data).unwrap();
                                                    println!("{:?}",s);
                                                    let channel = ev_loop.channel();
                                                    channel.send(s);
                                                    /*for (key,mut val) in self.socks {
                                                        if (c.header.uri == val.header.uri){
                                                            val.sock.write(v.data.clone().as_slice());
                                                        }
                                                    }*/
                                                    },
                                                Err(_) => panic!("Error dataframe")
                                            }
                                            println!("Connected");
                                            },
                                        _ => {
                                            let buf = &mut [0;1024];
                                            match c.sock.read(buf){
                                                Ok(n) => {
                                                    Parser::new().parser(buf,&mut c.header);
                                                    println!("Sock: {:?}",c.header.fields.get("Sec-WebSocket-Key"));
                                                    c.interest.remove(EventSet::readable());
                                                    c.interest.insert(EventSet::writable());
                                                    c.state = ClienState::HandshakeResponse;
                                                    ev_loop.reregister(&c.sock,token,c.interest,PollOpt::edge() | PollOpt::oneshot());
                                                },
                                                Err(_) => panic!("Read error")
                                            }
                                        }
                                //println!("{} - {:?}",n,std::str::from_utf8(buf).unwrap().to_string());
                                    }
                    },
                    _ => panic!("No sock in Hash")
                }
                println!("client: {:?}",client);
            }
        }
        }else{
            let c = self.socks.get_mut(&token).unwrap();
            let key = match c.header.fields.get("Sec-WebSocket-Key") {
                Some(v) => gen_key(v),
                _ => panic!("No key")
            };
            println!("Sock: {:?} - {:?}",c.header.fields.get("Sec-WebSocket-Key"),key);

            let resp = format!("HTTP/1.1 101 Switching Protocols\r\n\
            Upgrade: websocket\r\n\
            Connection: Upgrade\r\n\
            Sec-WebSocket-Accept: {}\r\n\
            Sec-WebSocket-Version: 13\r\n\r\n",key);
            let n = c.sock.write(resp.as_bytes());
            println!("Write: {:?}",n);
            c.interest.remove(EventSet::writable());
            c.interest.insert(EventSet::readable());
            c.state = ClienState::Connected;
            ev_loop.reregister(&c.sock, token, c.interest,
              PollOpt::edge() | PollOpt::oneshot()).unwrap();
            /*let buf = &mut [0;1024];
            match c.sock.read(buf){
                Ok(n) => {println!(" Buf {:?}",std::str::from_utf8(buf).unwrap().to_string())},
                Err(_) => panic!("Read error")
            };*/

        }
    }

    fn notify(&mut self, ev_loop: &mut EventLoop<MyHandler>, msg: String){
        println!("Message: {}",msg);
        for k in self.socks.keys() {
            match self.socks.get_mut(k) {
                &Some(&mut m) => {let mut s = m.sock; s.write("asas".as_bytes());},
                _ => panic!("popo")
            }
        }
            //if (c.header.uri == val.header.uri){
                //val.sock.write(v.data.clone().as_slice());
            //}
        //ev_loop.shutdown();
    }

    fn timeout(&mut self, ev_loop: &mut EventLoop<MyHandler>, timeout: u32){
        println!("Timeout: {}",timeout);
        ev_loop.shutdown();
    }
}

fn start_server (){
    let addr = HOST.parse().unwrap();
    let server = TcpListener::bind(&addr).unwrap();
    let mut ev_loop = match EventLoop::new(){
        Ok(ev) => ev,
        Err(er) => panic!("No event loop initialized: {:?}",er)
    };
    let channel = ev_loop.channel();
    channel.send("minchia".to_string());
    ev_loop.register(&server,SERVER);

    let timeout = ev_loop.timeout_ms(123,121000);

    ev_loop.run(&mut MyHandler::new(server)).unwrap();
}

fn main(){
    start_server();
}

#[cfg(test)]
mod test{
    use super::{start_server,HOST};
    use std::io::prelude::*;
    use std::net::TcpStream;
    use std::thread;

    fn test(s: &[u8]){

    }

    #[test]
    fn test_macro(){
        let b = b"GET / HTTP/1.1\r\nHost: 127.0.0.1:8003";
        test(b);
        //token!(b);
    }

    #[test]
    fn start() {
        thread::spawn(move || {start_server();}).join();
    }
    #[test]
    fn client(){
        thread::sleep_ms(3000);
        for n in 0..1 {
            thread::spawn(move || {match TcpStream::connect(HOST) {
                Ok(mut stream) => write_header(&mut stream),
                Err(err) => assert!(false)

            }});
        }


    }

    fn write_header(stream: &mut TcpStream){
        let req = b"GET / HTTP/1.1\r\nHost: 127.0.0.1:8003\r\nConnection: Upgrade\r\nPragma: no-cache\r\nCache-Control: no-cache\r\nUpgrade: websocket\r\nOrigin: http://localhost:3000\r\nSec-WebSocket-Version: 13\r\nUser-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_10_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/44.0.2403.130 Safari/537.36\r\nAccept-Encoding: gzip, deflate, sdch\r\nAccept-Language: en-US,en;q=0.8,it;q=0.6\r\nSec-WebSocket-Key: xr6Vrz9ltdC+EFfKPRQ4Bw==\r\nSec-WebSocket-Extensions: permessage-deflate; client_max_window_bits\r\n\r\n";
        let n = stream.write(req).unwrap();
        assert_eq!(n,req.len());
    }
}
