use std::io;

pub fn eof() -> io::Error {
  io::Error::new(io::ErrorKind::UnexpectedEof, "early eof")
}

pub fn crypto_error() -> io::Error {
  io::Error::new(io::ErrorKind::Other, "crypto error")
}