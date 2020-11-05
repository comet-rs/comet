use crate::crypto::*;
use crate::prelude::*;
use crate::utils::io::*;
use bytes::buf::Limit;
use futures::ready;
use std::cmp;
use std::pin::Pin;
use std::task::{Context, Poll};
use stream::StreamCipherKind;

pub fn register(plumber: &mut Plumber) {
  plumber.register("ss_stream_cipher_client", |conf| {
    let config: ClientConfig = from_value(conf)?;
    let method = config.method;
    let key = config.method.derive_key(config.password.as_str())?;

    Ok(Box::new(ClientProcessor {
      method,
      master_key: key,
    }))
  });
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum ShadowsocksStreamCipherKind {
  #[serde(rename = "aes-256-cfb")]
  Aes256Cfb,
}

impl Into<StreamCipherKind> for ShadowsocksStreamCipherKind {
  fn into(self) -> StreamCipherKind {
    match self {
      ShadowsocksStreamCipherKind::Aes256Cfb => StreamCipherKind::Aes256Cfb,
    }
  }
}

impl ShadowsocksStreamCipherKind {
  fn derive_key(&self, password: &str) -> Result<Bytes> {
    let cipher_kind: StreamCipherKind = (*self).into();
    hashing::evp_bytes_to_key(
      hashing::HashKind::Md5,
      password.as_ref(),
      cipher_kind.key_len(),
    )
    .into()
  }
  fn generate_salt(&self) -> Result<Bytes> {
    let cipher_kind: StreamCipherKind = (*self).into();
    let salt_len = cipher_kind.iv_len().unwrap();
    let mut salt = BytesMut::with_capacity(salt_len);
    unsafe {
      salt.set_len(salt_len);
    }

    rand::rand_bytes(&mut salt)?;
    Ok(salt.freeze())
  }
  fn to_crypter(
    &self,
    mode: CrypterMode,
    key: &[u8],
    salt: &[u8],
  ) -> Result<Box<dyn stream::StreamCrypter>> {
    let cipher_kind: stream::StreamCipherKind = (*self).into();
    cipher_kind.to_crypter(mode, key, salt).into()
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ClientConfig {
  method: ShadowsocksStreamCipherKind,
  password: SmolStr,
}

#[derive(Debug)]
pub struct ClientProcessor {
  method: ShadowsocksStreamCipherKind,
  master_key: Bytes,
}

#[async_trait]
impl Processor for ClientProcessor {
  async fn process(
    self: Arc<Self>,
    stream: RWPair,
    conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<RWPair> {
    let salt = self.method.generate_salt()?;
    let stream = ClientStream::new(stream, self.method, &self.master_key, &salt)?;
    conn.set_var("ss-salt", salt);
    conn.set_var("ss-key", self.master_key.clone());

    Ok(RWPair::new(stream))
  }
}

struct ClientStream<RW> {
  inner: RW,
  // Writing
  encrypter: Box<dyn stream::StreamCrypter>,
  write_state: WriteState,
  write_buf: BytesMut,
  // Reading
  read_state: ReadState,
}

impl<RW> ClientStream<RW> {
  fn new(
    inner: RW,
    method: ShadowsocksStreamCipherKind,
    master_key: &[u8],
    salt: &[u8],
  ) -> Result<Self> {
    let encrypter = method.to_crypter(CrypterMode::Encrypt, master_key, &salt)?;
    let mut buf = BytesMut::with_capacity(8192);
    buf.put_slice(&salt);

    Ok(Self {
      inner,

      encrypter,
      write_buf: buf,
      write_state: WriteState::Waiting,

      read_state: ReadState::ReadSalt {
        master_key: Bytes::copy_from_slice(master_key),
        method,
        salt_buf: BytesMut::with_capacity(16).limit(16),
      },
    })
  }
}

enum WriteState {
  Waiting,
  Writing { consumed: usize, written: usize },
}

impl<RW: AsyncWrite + Unpin> AsyncWrite for ClientStream<RW> {
  fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
    loop {
      match self.write_state {
        WriteState::Waiting => {
          let me = &mut *self;

          let consumed = cmp::min(buf.len(), me.write_buf.remaining_mut());
          assert!(consumed > 0);

          let old_len = me.write_buf.len();
          unsafe {
            me.write_buf.set_len(old_len + consumed);
          }
          let mut crypto_output = &mut me.write_buf[old_len..old_len + consumed];
          crypto_output.copy_from_slice(&buf[0..consumed]);

          let n = me
            .encrypter
            .update_in_place(&mut crypto_output)
            .map_err(|_| crypto_error())?;
          me.write_buf.truncate(old_len + n);

          self.write_state = WriteState::Writing {
            consumed,
            written: 0,
          };
        }
        WriteState::Writing {
          consumed,
          mut written,
        } => {
          let me = &mut *self;
          let n = ready!(Pin::new(&mut me.inner).poll_write(cx, &me.write_buf[written..]))?;

          written += n;
          if written >= me.write_buf.len() {
            // Writing complete
            me.write_state = WriteState::Waiting;
            me.write_buf.clear();
            return Poll::Ready(Ok(consumed));
          }
          self.write_state = WriteState::Writing { consumed, written };
        }
      }
    }
  }
  fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
    Pin::new(&mut self.inner).poll_flush(cx)
  }

  fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
    Pin::new(&mut self.inner).poll_shutdown(cx)
  }
}

enum ReadState {
  ReadSalt {
    master_key: Bytes,
    method: ShadowsocksStreamCipherKind,
    salt_buf: Limit<BytesMut>,
  },
  ReadData(Box<dyn stream::StreamCrypter>),
}

impl<RW: AsyncRead + Unpin> AsyncRead for ClientStream<RW> {
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    mut buf: &mut tokio::io::ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    let me = &mut *self;

    if buf.remaining() == 0 {
      return Poll::Ready(Ok(()));
    }

    loop {
      match &mut me.read_state {
        ReadState::ReadSalt {
          master_key,
          method,
          salt_buf,
        } => {
          let n = ready!(Pin::new(&mut me.inner).poll_read_buf(cx, salt_buf))?;
          if n == 0 {
            return Poll::Ready(Ok(()));
          }
          if !salt_buf.has_remaining_mut() {
            let dec = method
              .to_crypter(CrypterMode::Decrypt, &master_key, &salt_buf.get_ref())
              .map_err(|_| crypto_error())?;
            me.read_state = ReadState::ReadData(dec);
          }
        }
        ReadState::ReadData(dec) => {
          let filled_orig = buf.filled().len();
          ready!(Pin::new(&mut me.inner).poll_read(cx, &mut buf))?;

          if buf.filled().len() == filled_orig {
            // EOF
            return Poll::Ready(Ok(()));
          }

          let n = dec
            .update_in_place(&mut buf.filled_mut()[filled_orig..])
            .map_err(|_| crypto_error())?;
          debug_assert_eq!(n, buf.filled().len() - filled_orig);
          buf.set_filled(filled_orig + n);

          return Poll::Ready(Ok(()));
        }
      }
    }
  }
}