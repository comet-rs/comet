pub mod auth;
pub mod handshake;
pub mod obfs;
pub mod stream_cipher;

use crate::Plumber;
use handshake::ShadowsocksClientHandshakeProcessor;

enum ClientCipherProcessor {
  Stream(stream_cipher::ClientProcessor),
}

enum ClientProtocolProcessor {
  Origin,
  AuthAes128(auth::SsrClientAuthProcessor),
}

enum ClientObfsProcessor {
  Plain,
  Http(obfs::ClientProcessor),
}

pub struct SsrClientProcessor {
  obfs: ClientObfsProcessor,
  cipher: ClientCipherProcessor,
  protocol: ClientProtocolProcessor,
  handshake: ShadowsocksClientHandshakeProcessor,
}

pub fn register(plumber: &mut Plumber) {
  auth::register(plumber);
  handshake::register(plumber);
  obfs::register(plumber);
  stream_cipher::register(plumber);
}