use crate::crypto::stream::StreamCrypter;
use crate::prelude::*;
use crate::utils::io::*;
use crate::{check_eof, crypto::*};
use bytes::buf::Limit;
use futures::ready;
use std::cmp;
use std::pin::Pin;
use std::task::{Context, Poll};
use stream::StreamCipherKind;
use tokio_util::io::poll_read_buf;

pub fn register(plumber: &mut Plumber) {
    plumber.register("ss_stream_cipher_client", |conf, _| {
        let config: ClientConfig = from_value(conf)?;
        let processor = ClientProcessor::new(config.method, config.password.as_str());
        Ok(Box::new(processor))
    });
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum SsStreamCipherKind {
    #[serde(rename = "aes-128-cfb")]
    Aes128Cfb,
    #[serde(rename = "aes-192-cfb")]
    Aes192Cfb,
    #[serde(rename = "aes-256-cfb")]
    Aes256Cfb,
}

impl Into<StreamCipherKind> for SsStreamCipherKind {
    fn into(self) -> StreamCipherKind {
        match self {
            SsStreamCipherKind::Aes128Cfb => StreamCipherKind::Aes128Cfb,
            SsStreamCipherKind::Aes192Cfb => StreamCipherKind::Aes192Cfb,
            SsStreamCipherKind::Aes256Cfb => StreamCipherKind::Aes256Cfb,
        }
    }
}

impl SsStreamCipherKind {
    fn derive_key(&self, password: &str) -> Bytes {
        let cipher_kind: StreamCipherKind = (*self).into();
        hashing::evp_bytes_to_key(
            hashing::HashKind::Md5,
            password.as_ref(),
            cipher_kind.key_len(),
        )
    }

    fn generate_salt(&self) -> Result<Bytes> {
        let cipher_kind: StreamCipherKind = (*self).into();
        let salt_len = cipher_kind.iv_len();
        let mut salt = BytesMut::with_capacity(salt_len);
        unsafe {
            salt.set_len(salt_len);
        }

        random::rand_bytes(&mut salt)?;
        Ok(salt.freeze())
    }

    fn to_crypter(&self, mode: CrypterMode, key: &[u8], salt: &[u8]) -> Result<stream::SsCrypter> {
        let cipher_kind: stream::StreamCipherKind = (*self).into();
        cipher_kind.to_crypter(mode, key, salt)
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ClientConfig {
    method: SsStreamCipherKind,
    password: SmolStr,
}

#[derive(Debug)]
pub struct ClientProcessor {
    method: SsStreamCipherKind,
    master_key: Bytes,
}

impl ClientProcessor {
    pub fn new(method: SsStreamCipherKind, password: &str) -> Self {
        let key = method.derive_key(password);
        Self {
            method,
            master_key: key,
        }
    }
}

#[async_trait]
impl Processor for ClientProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let stream = stream.into_tcp()?;
        let salt = self.method.generate_salt()?;

        let stream = ClientStream::new(stream, self.method, &self.master_key, &salt)?;

        conn.set_var(vars::SS_KEY, self.master_key.clone());
        conn.set_var(vars::SS_SALT, salt);

        Ok(RWPair::new(stream).into())
    }
}

struct ClientStream<RW> {
    inner: RW,
    // Writing
    encrypter: stream::SsCrypter,
    write_state: WriteState,
    write_buf: BytesMut,
    // Reading
    read_state: ReadState,
}

impl<RW> ClientStream<RW> {
    fn new(inner: RW, method: SsStreamCipherKind, master_key: &[u8], salt: &[u8]) -> Result<Self> {
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
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<IoResult<usize>> {
        loop {
            match self.write_state {
                WriteState::Waiting => {
                    let me = &mut *self;

                    let consumed = cmp::min(buf.len(), me.write_buf.remaining_mut());
                    assert!(consumed > 0);

                    let old_len = me.write_buf.len();
                    me.write_buf.extend_from_slice(&buf[0..consumed]);
                    let mut crypto_output = &mut me.write_buf[old_len..old_len + consumed];

                    let n = me
                        .encrypter
                        .update(&mut crypto_output)
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
                    let n =
                        ready!(Pin::new(&mut me.inner).poll_write(cx, &me.write_buf[written..]))?;

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
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

enum ReadState {
    ReadSalt {
        master_key: Bytes,
        method: SsStreamCipherKind,
        salt_buf: Limit<BytesMut>,
    },
    ReadData(stream::SsCrypter),
}

impl<RW: AsyncRead + Unpin> AsyncRead for ClientStream<RW> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
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
                    check_eof!(ready!(poll_read_buf(
                        Pin::new(&mut me.inner),
                        cx,
                        salt_buf
                    ))?);
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
                        .update(&mut buf.filled_mut()[filled_orig..])
                        .map_err(|_| crypto_error())?;
                    debug_assert_eq!(n, buf.filled().len() - filled_orig);
                    buf.set_filled(filled_orig + n);

                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
}
