// `use std::net::TcpStream;` en commentaire ne doit pas compter.
use std::{fs::File, io::Read};

pub fn load(path: &str) -> String {
    let mut buffer = String::new();
    File::open(path).unwrap().read_to_string(&mut buffer).unwrap();
    buffer
}
