
#[macro_use]
extern crate log;
extern crate fern;
extern crate time;
extern crate punpun;

use punpun::engne::server::{start_server};


use std::collections::BTreeMap;

use log::*;

use std::io;





const HOST: &'static str = "0.0.0.0:10001";
const LOG_LEVEL: log::LogLevelFilter = log::LogLevelFilter::Debug;
const LOG_FILE_NAME: &'static str = "/Users/sommoyogurt/workspace/rust/punpun/logs/debug.log";



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




fn main() {
    match init_logger(){
        Ok(s) => info!("Logger init"),
        Err(e) => error!("Error init logger")
    }
    start_server(HOST);
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
