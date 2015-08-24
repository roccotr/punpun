
extern crate sha1;
extern crate rustc_serialize;


use self::rustc_serialize::base64::{ToBase64, STANDARD};


const MAGIC_WS: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";


pub fn gen_key(key: &String) -> String {
	let mut m = sha1::Sha1::new();
	let mut buf = [0u8;20];

	m.update(key.as_bytes());
	m.update(MAGIC_WS.as_bytes());

	m.output(&mut buf);

	return buf.to_base64(STANDARD);
}
