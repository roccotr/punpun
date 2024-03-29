
use mio::{Token, EventSet, EventLoop, PollOpt, TryRead, TryWrite};
use mio::buf::ByteBuf;

use std::io::{Result, Error, ErrorKind};

use ::engne::server::Server;

use mio::tcp::*;



pub struct Connection {
    sock: TcpStream,
    pub token: Token,
    interest: EventSet,
    send_queue: Vec<ByteBuf>,
}

impl Connection {
    pub fn new(sock: TcpStream, token: Token) -> Connection {
        Connection {
            sock: sock,
            token: token,
            interest: EventSet::hup(),
            send_queue: Vec::new(),
        }
    }

    pub fn readable(&mut self) -> Result<ByteBuf> {
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

    pub fn writable(&mut self) -> Result<()> {

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

    pub fn send_message(&mut self, message: ByteBuf) -> Result<()> {
        self.send_queue.push(message);
        self.interest.insert(EventSet::writable());
        Ok(())
    }
    pub fn register(&mut self, event_loop: &mut EventLoop<Server>) -> Result<()> {
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

    pub fn reregister(&mut self, event_loop: &mut EventLoop<Server>) -> Result<()> {
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
