
#[macro_use]
extern crate bitflags;
extern crate byteorder;
extern crate rand;
extern crate openssl;

pub mod engne;
pub mod hlprs;

/*
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate mio;

pub mod engne;
pub mod hlprs;




use mio::*;
use mio::tcp::*;


struct Server;


impl Handler for Server {
type Timeout = usize;
type Message = ();

fn ready(&mut self,event_loop: &mut EventLoop<Self>, token: Token, events: EventSet){
debug!("Ready: {:?}",token);

}
}

fn run(){
let address: &'static str = "0.0.0.0:10005";

let addr = std::str::FromStr::from_str(&address).unwrap();

// Open socket
let server_socket = TcpSocket::v4().unwrap();
server_socket.bind(&addr).ok().expect("Could not connect to address: 0.0.0.0:10005");

let mut event_loop = mio::EventLoop::new().unwrap();



let mut server = Server;
let server_listener = server_socket.listen(256).unwrap();

//let mut server = Server::new(server_socket);
//println!("Run server on {}",addr);
info!("Listen on 0.0.0.0:10005");
event_loop.register_opt(&server_listener,Token(0),mio::EventSet::readable(),mio::PollOpt::edge()).unwrap();
event_loop.run(&mut server).unwrap();
}



static ADDRESS : &'static str = "0.0.0.0:100005";

fn get_stream(address: &'static str) -> std::io::Result<std::net::TcpStream> {
	let addr: std::net::SocketAddr = std::str::FromStr::from_str(&address).unwrap();
	std::net::TcpStream::connect(&addr)
}



#[test]
fn client(){
env_logger::init().unwrap();
	debug!("run");

	let t = std::thread::spawn( move || {	run();});
debug!("run");

	let client = get_stream(ADDRESS);

std::thread::sleep_ms(12000);

	match client {
			Ok(stream) => assert!(true),
			Err(_) => assert!(false)
		}

	t.join();

}*/
