
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate sha1;
extern crate rustc_serialize;
extern crate slab;




//extern crate punpun;


use std::collections::HashMap;
use std::io::prelude::*;
use std::thread;
use std::net;
use std::io::BufReader;
use self::rustc_serialize::base64::{ToBase64, STANDARD};
use slab::*;
//use punpun::engne::server::*;

const HOST: &'static str = "0.0.0.0:10001";

#[derive(Debug)]
struct Client{
    key: Key,
    sock: net::TcpStream,
    addr: net::SocketAddr
}

impl Client{
    fn new(key: Key, sock: net::TcpStream, addr: net::SocketAddr) -> Client {
        Client{key: key, sock: sock, addr: addr}
    }
    fn read(&self){

    }
}


#[derive(Debug,Clone)]
struct Key {
    i: usize
}


impl Index for Key {
    fn from_usize(i: usize) -> Key{
        Key{i:i}
    }
    fn as_usize(&self) ->  usize {
        self.i
    }
}


#[derive(Debug)]
struct Server {
    sock: net::TcpListener,
    socks: Slab<Client,Key>,
}

trait Handler {
    fn run(&mut self, ev_loop: &mut EvLoop<Self>);
    fn ready(&mut self,key: Key, ev: &mut EvLoop<Self>);
}

impl Server {
    fn new(sock: net::TcpListener) -> Server {
        Server{sock: sock, socks: Slab::<Client,Key>::new_starting_at(Key{i:1 as usize}, 10)}
    }
}

impl Handler for Server {
    fn run(&mut self, ev_loop: &mut EvLoop<Server>) {
        loop {
            match self.sock.accept(){
            Ok((stream,addr)) => {
                    match self.socks.insert_with(move |key| {
                        debug!("Register new connection: {:?}",key);
                        Client::new(key,stream,addr)
                        }){
                            Some(i) => {
                                debug!("I: {:?}",self.socks.get(Key{i:1 as usize}));
                                let key = i.clone();
                                match self.socks.get_mut(i){
                                    Some(mut client) => {debug!("Register client event");ev_loop.register(key,&mut client.sock,&mut ev_loop)},
                                    None => error!("No client found")
                                };
                                        },
                            None => error!("No Register")
                        };
                },
                Err(e) => error!("New connection error")
            }
        }
    }
    fn ready(&mut self,key: Key, ev: &mut EvLoop<Self>){

    }
}

#[derive(Clone)]
struct EvLoop<T: Handler>{
    handler: T
}

impl <T: Handler> EvLoop<T> {

    fn new(handler: T) -> EvLoop<T> where T: Handler{
        EvLoop::<T>{handler:handler}
    }

    fn run(&mut self){
        self.handler.ready(Key{i:1 as usize});
    }

    fn register(&self, key: Key, sock: &mut net::TcpStream, ev_loop: &mut EvLoop<Server>){
        let mut s = sock.try_clone().unwrap();
        let i = key.clone();
        thread::spawn(move || {
            let mut buf = [0u8;512];
            match s.read(&mut buf){
                 Ok(n) => {debug!("Read {:?}: {} - {}",s.peer_addr(),n,std::str::from_utf8(&buf).unwrap().to_string());},
                 Err(e) => error!("Error read, {:?}",e)
             }
        });

    }
}

fn start_server(){
    env_logger::init().unwrap();
    let listener = net::TcpListener::bind(HOST).unwrap();

    let mut server = Server::new(listener);

    let mut ev_loop = EvLoop::new(server);
    info!("Start server on: {}",HOST);
    ev_loop.run();
}


fn main(){
    start_server();
}

#[cfg(test)]
mod test{
    use super::{start_server,HOST};
    use std::thread;
    use std::net::{TcpListener,TcpStream,Shutdown};
    use std::io::Write;

    #[test]
    fn start(){
        start_server();
    }

    #[test]
    fn connect(){
        thread::sleep_ms(3000);
        for i in 0..10{
            client();
        }
    }

    fn client(){
        debug!("test 2");
        let mut stream = TcpStream::connect(HOST).unwrap();
        stream.write("Hello".as_bytes());
        stream.shutdown(Shutdown::Write);
    }
}
