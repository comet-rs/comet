use crate::crypto::*;
use crate::prelude::*;
use crate::utils::io::*;
use futures::ready;
use pin_project_lite::pin_project;
use std::cmp;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::AsyncBufRead;
use tokio::io::ReadBuf;

pub mod auth;
pub mod handshake;

#[derive(Deserialize, Debug, Clone)]
pub struct ShadowsocksClientConfig {
  method: ShadowsocksCipherKind,
  password: SmolStr,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum ShadowsocksCipherKind {
  #[serde(rename = "aes-256-cfb")]
  Aes256Cfb,
}

impl Into<CipherKind> for ShadowsocksCipherKind {
  fn into(self) -> CipherKind {
    match self {
      ShadowsocksCipherKind::Aes256Cfb => CipherKind::Stream(stream::StreamCipherKind::Aes256Cfb),
    }
  }
}

impl ShadowsocksCipherKind {
  fn derive_key(&self, password: &str) -> Result<Bytes> {
    let cipher_kind: CipherKind = (*self).into();
    match cipher_kind {
      CipherKind::Stream(bc) => Ok(hashing::evp_bytes_to_key(
        hashing::HashKind::Md5,
        password.as_ref(),
        bc.key_len(),
      )?),
    }
  }
  fn generate_salt(&self) -> Result<Bytes> {
    match (*self).into() {
      CipherKind::Stream(bc) => {
        let salt_len = bc.iv_len().unwrap();
        let mut salt = BytesMut::with_capacity(salt_len);
        unsafe {
          salt.set_len(salt_len);
        }

        rand::rand_bytes(&mut salt)?;
        Ok(salt.freeze())
      }
    }
  }
}

pub enum ShadowsocksCrypter {
  Stream(Box<dyn stream::StreamCrypter + Send + 'static>),
}

impl ShadowsocksCrypter {
  pub fn new(
    mode: CrypterMode,
    method: ShadowsocksCipherKind,
    master_key: &[u8],
    salt: &[u8],
  ) -> Result<Self> {
    let cipher_kind: CipherKind = method.into();
    match cipher_kind {
      CipherKind::Stream(bc) => Ok(ShadowsocksCrypter::Stream(
        bc.to_crypter(mode, master_key, salt)?,
      )),
    }
  }
}

pub struct ShadowsocksClientProcessor {
  method: ShadowsocksCipherKind,
  master_key: Bytes,
}

impl ShadowsocksClientProcessor {
  pub fn new(config: &ShadowsocksClientConfig) -> Result<Self> {
    let method = config.method;
    let key = config.method.derive_key(config.password.as_str())?;
    println!("{:?}", key);
    Ok(ShadowsocksClientProcessor {
      method,
      master_key: key,
    })
  }
}

#[async_trait]
impl Processor for ShadowsocksClientProcessor {
  async fn process(
    self: Arc<Self>,
    stream: RWPair,
    _conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<RWPair> {
    let writer = ShadowsocksEncryptWriter::new(stream.write_half, self.method, &self.master_key)?;
    let reader = ShadowsocksDecryptReader::new(stream.read_half, self.method, &self.master_key);
    Ok(RWPair::new_parts(reader, writer))
  }
}

enum WriteState {
  Waiting,
  Writing { consumed: usize, written: usize },
}

struct ShadowsocksEncryptWriter<W> {
  encrypter: ShadowsocksCrypter,
  inner: W,
  state: WriteState,
  buf: BytesMut,
}

impl<W: AsyncWrite + Unpin> ShadowsocksEncryptWriter<W> {
  fn new(writer: W, method: ShadowsocksCipherKind, master_key: &[u8]) -> Result<Self> {
    let salt = method.generate_salt()?;
    let encrypter = ShadowsocksCrypter::new(CrypterMode::Encrypt, method, master_key, &salt)?;
    let mut buf = BytesMut::with_capacity(8192);
    buf.put_slice(&salt);

    Ok(Self {
      encrypter: encrypter,
      inner: writer,
      buf: buf,
      state: WriteState::Waiting,
    })
  }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for ShadowsocksEncryptWriter<W> {
  fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
    loop {
      match self.state {
        WriteState::Waiting => {
          let me = &mut *self;

          let consumed = cmp::min(buf.len(), me.buf.remaining_mut());
          assert!(consumed > 0);

          let old_len = me.buf.len();
          unsafe {
            me.buf.set_len(old_len + consumed);
          }
          let crypto_output = &mut me.buf[old_len..old_len + consumed];

          match &mut me.encrypter {
            ShadowsocksCrypter::Stream(bc) => {
              let n = bc
                .update(&buf[0..consumed], crypto_output)
                .map_err(|_| crypto_error())?;
              me.buf.truncate(old_len + n);
            }
          }
          self.state = WriteState::Writing {
            consumed,
            written: 0,
          };
        }
        WriteState::Writing {
          consumed,
          mut written,
        } => {
          let me = &mut *self;
          let n = ready!(Pin::new(&mut me.inner).poll_write(cx, &me.buf[written..]))?;

          written += n;
          if written >= me.buf.len() {
            // Writing complete
            me.state = WriteState::Waiting;
            me.buf.clear();
            return Poll::Ready(Ok(consumed));
          }
          self.state = WriteState::Writing { consumed, written };
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
    method: ShadowsocksCipherKind,
  },
  ReadData(ShadowsocksCrypter),
}
pin_project! {
  struct ShadowsocksDecryptReader<R> {
    #[pin]
    inner: R,
    state: ReadState,
    buf: BytesMut,
  }
}

impl<R: AsyncRead> ShadowsocksDecryptReader<R> {
  fn new(reader: R, method: ShadowsocksCipherKind, master_key: &[u8]) -> Self {
    Self {
      inner: reader,
      state: ReadState::ReadSalt {
        master_key: Bytes::copy_from_slice(master_key),
        method,
      },
      buf: BytesMut::with_capacity(8192),
    }
  }
}

impl<R: AsyncRead + Unpin> AsyncRead for ShadowsocksDecryptReader<R> {
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    const SALT_SIZE: usize = 16;
    let me = &mut *self;

    if buf.remaining() == 0 {
      return Poll::Ready(Ok(()));
    }

    loop {
      match &mut me.state {
        ReadState::ReadSalt { master_key, method } => {
          ready!(Pin::new(&mut me.inner).poll_read_buf(cx, &mut me.buf))?;
          if me.buf.len() >= SALT_SIZE {
            let salt = me.buf.split_to(SALT_SIZE);
            let dec = ShadowsocksCrypter::new(CrypterMode::Decrypt, *method, &master_key, &salt)
              .map_err(|_| crypto_error())?;
            me.state = ReadState::ReadData(dec);
          }
        }
        ReadState::ReadData(ref mut dec) => {
          if me.buf.is_empty() {
            let n = ready!(Pin::new(&mut me.inner).poll_read_buf(cx, &mut me.buf))?;
            if n == 0 {
              // EOF
              return Poll::Ready(Ok(()));
            }
          }
          let consumed = cmp::min(me.buf.len(), buf.remaining());
          unsafe {
            buf.assume_init(consumed);
          }

          match dec {
            ShadowsocksCrypter::Stream(bc) => {
              let n = bc
                .update(&me.buf[..consumed], &mut buf.initialized_mut()[..consumed])
                .map_err(|_| crypto_error())?;
              assert_eq!(n, consumed);
              buf.set_filled(n);
            }
          }

          me.buf.advance(consumed);
          return Poll::Ready(Ok(()));
        }
      }
    }
  }
}
