mod alter_id;
mod crypto;
mod session;

use crate::{delegate_read, delegate_write_all, utils::io::eof};
use std::cmp::min;

use crate::{
    crypto::aead::{self, AeadCrypter},
    crypto::CrypterMode,
    utils::io::io_other_error,
};
use bytes::BufMut;
use futures::ready;
use tokio_util::io::poll_read_buf;
use uuid::Uuid;

use crate::prelude::*;

use self::{
    alter_id::UserId,
    crypto::{ShakeGenerator, VmessNonceSeq},
    session::ClientSession,
};

const MAX_LEN: usize = 16384;
const MAX_PADDING_LEN: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum SecurityType {
    #[serde(rename = "aes-128-gcm")]
    Aes128Gcm,
    #[serde(rename = "chacha20-poly1305")]
    Chacha20Poly1305,
    #[serde(rename = "auto")]
    Auto,
}

impl Default for SecurityType {
    fn default() -> Self {
        if cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
            Self::Aes128Gcm
        } else {
            Self::Chacha20Poly1305
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ClientConfig {
    user_id: Uuid,
    #[serde(default)]
    alter_id: u16,
    #[serde(default)]
    security: SecurityType,
}

struct ClientProcessor {
    security: SecurityType,
    accounts: Vec<UserId>,
}

impl ClientProcessor {
    fn new(mut config: ClientConfig) -> Self {
        if config.security == SecurityType::Auto {
            config.security = SecurityType::default();
        }
        let primary = alter_id::UserId::new(config.user_id);

        let mut accounts = if config.alter_id > 0 {
            alter_id::new_alter_ids(&primary, config.alter_id)
        } else {
            vec![]
        };
        accounts.push(primary);

        Self {
            security: config.security,
            accounts,
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

        let user = &self.accounts[0];
        let session = Arc::new(ClientSession::new(user));
        let header = session.encode_request_header(self.security, conn)?;

        let reader = ClientReader::new(stream, session.clone(), self.security)?;
        let writer = ClientWriter::new(reader, session.clone(), self.security, header)?;
        Ok(RWPair::new(writer).into())
    }
}

struct ClientReader<R> {
    inner: R,
    security: SecurityType,
    state: ClientReaderState,
    read_buf: BytesMut,
    shake: ShakeGenerator,
    crypter: aead::SsCrypter<VmessNonceSeq>,
}

impl<R: AsyncRead + Unpin> ClientReader<R> {
    fn new(inner: R, session: Arc<ClientSession>, security: SecurityType) -> Result<Self> {
        let read_buf = BytesMut::with_capacity(4 + 2);
        let shake = ShakeGenerator::new(&session.response_iv);
        let nonce_seq = VmessNonceSeq::new(&session.response_iv[2..12]);
        let crypter = match security {
            SecurityType::Aes128Gcm => aead::AeadCipherKind::Aes128Gcm.to_crypter(
                CrypterMode::Decrypt,
                &session.response_key,
                nonce_seq,
            )?,
            SecurityType::Chacha20Poly1305 => {
                let key = crypto::generate_chacha20poly1305_key(&session.response_key);
                aead::AeadCipherKind::Chacha20Poly1305.to_crypter(
                    CrypterMode::Decrypt,
                    &key,
                    nonce_seq,
                )?
            }
            SecurityType::Auto => unimplemented!(),
        };
        let state = ClientReaderState::ReadHeader(session);

        Ok(Self {
            inner,
            security,
            state,
            read_buf,
            shake,
            crypter,
        })
    }

    fn poll_fill_at_least(
        &mut self,
        cx: &mut std::task::Context<'_>,
        len: usize,
    ) -> Poll<IoResult<()>> {
        let rem = self.read_buf.remaining_mut();
        if rem < len {
            self.read_buf.reserve(len - rem);
        }

        if ready!(poll_read_buf(
            Pin::new(&mut self.inner),
            cx,
            &mut self.read_buf
        ))? == 0
        {
            Poll::Ready(Err(eof()))
        } else {
            Poll::Ready(Ok(()))
        }
    }
}

enum ClientReaderState {
    ReadHeader(Arc<ClientSession>),
    ReadLength,
    ReadData { length: usize, padding: usize },
    ConsumeData { buf: BytesMut },
}

impl<R: AsyncRead + Unpin> AsyncRead for ClientReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }

        let me = &mut *self;
        loop {
            match &mut me.state {
                ClientReaderState::ReadHeader(sess) => {
                    if me.read_buf.len() >= 4 {
                        let header = me.read_buf.split_to(4);
                        sess.decode_response_header(&header)
                            .map_err(|e| io_other_error(e))?;

                        me.state = ClientReaderState::ReadLength;
                    }

                    ready!(me.poll_fill_at_least(cx, 4))?;
                }
                ClientReaderState::ReadLength => {
                    if me.read_buf.len() >= 2 {
                        // Order matters, read padding size first.
                        let padding = me.shake.next_padding();
                        let length = me.shake.encode(me.read_buf.get_u16()) as usize;

                        me.state = ClientReaderState::ReadData { length, padding };
                        continue;
                    }

                    ready!(me.poll_fill_at_least(cx, 2))?;
                }
                ClientReaderState::ReadData { length, padding } => {
                    if me.read_buf.len() >= *length {
                        // Data + Tag
                        let data_len = *length - *padding;

                        // Data only
                        let dec_len = me
                            .crypter
                            .update(&mut me.read_buf[..data_len])
                            .map_err(|e| io_other_error(e))?;

                        let mut decrypted = me.read_buf.split_to(data_len);
                        decrypted.truncate(dec_len);
                        me.read_buf.advance(data_len); // Consume padding

                        me.state = ClientReaderState::ConsumeData { buf: decrypted };
                        continue;
                    }

                    let length = *length;
                    ready!(me.poll_fill_at_least(cx, length))?;
                }
                ClientReaderState::ConsumeData { buf } => {
                    if me.read_buf.is_empty() {
                        me.state = ClientReaderState::ReadLength;
                        continue;
                    }

                    let len = min(me.read_buf.len(), buf.remaining());
                    let consumed = me.read_buf.split_to(len);
                    buf.put_slice(&consumed);

                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
}

delegate_write_all!(ClientReader);

struct ClientWriter<W> {
    inner: W,
    security: SecurityType,
    state: ClientWriterState,
    write_buf: BytesMut,
    shake: ShakeGenerator,
    crypter: aead::SsCrypter<VmessNonceSeq>,
}

impl<W: AsyncWrite + Unpin> ClientWriter<W> {
    fn new(
        inner: W,
        session: Arc<ClientSession>,
        security: SecurityType,
        header: BytesMut,
    ) -> Result<Self> {
        let write_buf = header;
        let shake = ShakeGenerator::new(&session.request_iv);
        let nonce_seq = VmessNonceSeq::new(&session.request_iv[2..12]);
        let crypter = match security {
            SecurityType::Aes128Gcm => aead::AeadCipherKind::Aes128Gcm.to_crypter(
                CrypterMode::Encrypt,
                &session.response_key,
                nonce_seq,
            )?,
            SecurityType::Chacha20Poly1305 => {
                let key = crypto::generate_chacha20poly1305_key(&session.request_key);
                aead::AeadCipherKind::Chacha20Poly1305.to_crypter(
                    CrypterMode::Encrypt,
                    &key,
                    nonce_seq,
                )?
            }
            SecurityType::Auto => unimplemented!(),
        };
        let state = ClientWriterState::Waiting;

        Ok(Self {
            inner,
            security,
            state,
            write_buf,
            shake,
            crypter,
        })
    }
}

enum ClientWriterState {
    Waiting,
    Writing { consumed: usize, written: usize },
}

impl<W: AsyncWrite + Unpin> AsyncWrite for ClientWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, futures_io::Error>> {
        if buf.len() == 0 {
            return Poll::Ready(Ok(0));
        }

        let me = &mut *self;
        loop {
            match &mut me.state {
                ClientWriterState::Waiting => {
                    let consumed = min(buf.len(), MAX_LEN - me.shake.max_padding());
                    let tag_len = me.crypter.tag_len();

                    let padding = me.shake.next_padding();
                    let length_enc = me.shake.encode(consumed as u16);

                    let old_len = me.write_buf.len();
                    me.write_buf.reserve(2 + consumed + tag_len + padding);
                    me.write_buf.put_u16(length_enc);
                    me.write_buf.extend_from_slice(&buf[0..consumed]);
                    unsafe {
                        me.write_buf.advance_mut(tag_len + padding);
                    }

                    let mut crypto_output =
                        &mut me.write_buf[old_len..old_len + consumed + tag_len];
                    me.crypter
                        .update(&mut crypto_output)
                        .map_err(|e| io_other_error(e))?;

                    me.state = ClientWriterState::Writing {
                        consumed,
                        written: 0,
                    };
                }
                ClientWriterState::Writing { consumed, written } => {
                    let n =
                        ready!(Pin::new(&mut me.inner).poll_write(cx, &me.write_buf[*written..]))?;

                    *written += n;
                    if *written >= me.write_buf.len() {
                        // Writing complete
                        let consumed = *consumed;
                        me.state = ClientWriterState::Waiting;
                        me.write_buf.clear();
                        return Poll::Ready(Ok(consumed));
                    }
                }
            }
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), futures_io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), futures_io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

delegate_read!(ClientWriter);
