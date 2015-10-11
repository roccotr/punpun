
use rustc_serialize::json::{self,Json,Encodable,Decodable};




#[derive(RustcEncodable, RustcDecodable, PartialEq, Debug)]
pub struct Message {
    action: String,
    channel: String,
    message: String,
    time: String,
}
