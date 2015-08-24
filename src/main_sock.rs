
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate sha1;
extern crate rustc_serialize;




//extern crate punpun;


use std::collections::HashMap;
use std::io::prelude::*;
use std::thread;
use std::net;
use std::io::BufReader;
use self::rustc_serialize::base64::{ToBase64, STANDARD};
//use punpun::engne::server::*;

const MAGIC_WS: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const HOST: &'static str = "127.0.0.1:8003";
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
#[derive(PartialEq)]
enum ClienState {
    AwaitingHandshake,
    HandshakeResponse,
    Connected
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
struct MyHandlerClient{
    sock: net::TcpStream,
    header: Header,
    state: ClienState,
}

impl MyHandlerClient{
    fn new(sock: net::TcpStream) -> MyHandlerClient {
        MyHandlerClient{sock:sock, header: Header::new(),state: ClienState::AwaitingHandshake}
    }
}

#[derive(Debug)]
struct MyHandler {
    i: usize,
    socks: HashMap<i32,MyHandlerClient>,
    server: net::TcpListener,
}



fn main(){

	env_logger::init().unwrap();
	let mut listener = net::TcpListener::bind("127.0.0.1:10001").unwrap();
	//let (tx,rx) = thread.channels();
	for stream in listener.incoming() {
		thread::spawn(move || {
			match stream {
				Ok(mut s) => {
					let mut client = MyHandlerClient::new(s);
					let mut header = Header::new();
					let mut buf = [0u8;512];
					let read = client.sock.read(&mut buf);
					match read {
						Ok(r) => {
							std::thread::spawn(move || {
								Parser::new().parser(&buf,&mut header);
								debug!("Sock: {:?}",header.fields.get("Sec-WebSocket-Key"));
								match header.fields.get("Sec-WebSocket-Key") {
									Some(k) => {
										client.state = ClienState::HandshakeResponse;
										let key = gen_key(k);
										let resp = format!("HTTP/1.1 101 Switching Protocols\r\n\
													Upgrade: websocket\r\n\
													Connection: Upgrade\r\n\
													Sec-WebSocket-Accept: {}\r\n\
													Sec-WebSocket-Version: 13\r\n\r\n",key);
										client.sock.write(resp.as_bytes());
										client.sock.flush();
										client.state = ClienState::Connected;
										let mut buf = [0u8;512];
										let read = client.sock.read(&mut buf);
										net::TcpStream::connect(s.addr);
										debug!("Qui read");


									},
									_ => panic!("No Key")
								}
								//debug!("{:?}",std::str::from_utf8(&buf).unwrap().to_string());
								});
						},
						Err(_) => println!("Error read")
					}
				},
				Err(_) => println!("Error stream")
			}
		});
	}
}
