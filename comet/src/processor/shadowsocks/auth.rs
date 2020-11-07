use super::handshake::ShadowsocksClientHandshakeProcessor;
use crate::check_eof;
use crate::crypto::rand::xor_rng;
use crate::crypto::*;
use crate::prelude::*;
use crate::utils::io::*;
use crate::utils::unix_ts;
use anyhow::anyhow;
use base64::encode_config_buf;
use futures::ready;
use std::cmp;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};
use xorshift::Rng;

const PACK_UNIT_SIZE: usize = 2000;

pub fn register(plumber: &mut Plumber) {
  plumber.register("ssr_auth_client", |conf| {
    let config: SsrClientAuthConfig = from_value(conf)?;
    let user_key = config.user_key.as_ref().map(|key| match config.protocol {
      SsrClientAuthType::AuthAes128Md5 => {
        hashing::hash_bytes(hashing::HashKind::Md5, key.as_bytes()).unwrap()
      }
      SsrClientAuthType::AuthAes128Sha1 => {
        hashing::hash_bytes(hashing::HashKind::Sha1, key.as_bytes()).unwrap()
      }
    });

    Ok(Box::new(SsrClientAuthProcessor {
      ids: Mutex::new(SsrIds::new()),
      user_id: config.user_id,
      protocol: config.protocol,
      user_key,
    }))
  });
}

#[derive(Debug)]
struct SsrIds {
  client_id: u32,
  connection_id: u32,
}

impl SsrIds {
  fn new() -> Self {
    let mut me = SsrIds {
      client_id: 0,
      connection_id: 0,
    };
    me.reset();
    me
  }

  fn reset(&mut self) {
    let mut rng = xor_rng();
    self.client_id = rng.gen();
    self.connection_id = rng.gen_range(0, 0xFFFFFF);
  }

  fn new_connection(&mut self) -> (u32, u32) {
    if self.connection_id >= 0xFF000000 {
      self.reset();
    }
    self.connection_id += 1;
    (self.client_id, self.connection_id)
  }
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum SsrClientAuthType {
  #[serde(rename = "auth_aes128_md5")]
  AuthAes128Md5,
  #[serde(rename = "auth_aes128_sha1")]
  AuthAes128Sha1,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SsrClientAuthConfig {
  user_id: Option<u32>,
  protocol: SsrClientAuthType,
  user_key: Option<SmolStr>,
}

#[derive(Debug)]
pub struct SsrClientAuthProcessor {
  ids: Mutex<SsrIds>,
  user_id: Option<u32>,
  user_key: Option<Bytes>,
  protocol: SsrClientAuthType,
}

impl SsrClientAuthProcessor {
  fn new_connection(&self) -> (u32, u32) {
    let mut ids = self.ids.lock().unwrap();
    ids.new_connection()
  }
}

#[async_trait]
impl Processor for SsrClientAuthProcessor {
  async fn process(
    self: Arc<Self>,
    stream: RWPair,
    conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<RWPair> {
    let write_key: &Bytes = conn
      .get_var("ss-key")
      .ok_or_else(|| anyhow!("Key not found"))?;
    let write_iv: &Bytes = conn
      .get_var("ss-salt")
      .ok_or_else(|| anyhow!("WriteIV not found"))?;

    let stream = AuthAes128ClientStream::new(
      stream,
      self.clone(),
      self.user_key.as_ref().unwrap_or(write_key).clone(),
      write_key.clone(),
      match self.protocol {
        SsrClientAuthType::AuthAes128Md5 => hashing::HashKind::Md5,
        SsrClientAuthType::AuthAes128Sha1 => hashing::HashKind::Sha1,
      },
      write_iv.clone(),
    );

    Ok(RWPair::new(stream))
  }
}

enum WriteState {
  PreparingData,
  Writing { chunks: VecDeque<Bytes> },
}

enum ReadState {
  Size,
  Random { rnd_len: usize, payload_len: usize },
  Data { payload_len: usize },
  Hmac,
}

struct AuthAes128ClientStream<RW> {
  inner: RW,
  // Writing
  processor: Arc<SsrClientAuthProcessor>,
  user_key: Bytes,
  write_key: Bytes,
  write_chunk_id: u32,
  header_sent: bool,
  write_iv: Bytes,
  hash_kind: hashing::HashKind,
  write_state: WriteState,
  last_write_len: usize,
  // Reading
  read_state: ReadState,
  read_buf: BytesMut,
}

impl<RW> AuthAes128ClientStream<RW> {
  fn new(
    inner: RW,
    processor: Arc<SsrClientAuthProcessor>,
    user_key: Bytes,
    write_key: Bytes,
    hash_kind: hashing::HashKind,
    write_iv: Bytes,
  ) -> Self {
    Self {
      inner,
      processor,
      user_key,
      write_key,
      write_chunk_id: 0,
      header_sent: false,
      write_iv,
      hash_kind,
      write_state: WriteState::PreparingData,
      read_state: ReadState::Size,
      read_buf: BytesMut::with_capacity(1500),
      last_write_len: 0,
    }
  }
  fn pack_auth_data(&self, buf: &[u8]) -> Result<Bytes> {
    let mut rng = xor_rng();

    let mut part12_hmac_key = BytesMut::with_capacity(self.user_key.len() + self.write_iv.len());
    part12_hmac_key.extend_from_slice(&self.write_iv);
    part12_hmac_key.extend_from_slice(&self.write_key);

    // Part 2-1
    let rnd_len = if buf.len() > 400 {
      rng.gen::<u16>() % 512
    } else {
      rng.gen::<u16>() % 1024
    };
    let pack_len = 7 + 24 + rnd_len + buf.len() as u16 + 4;

    // Returned buffer
    let mut ret = BytesMut::with_capacity(pack_len as usize);

    // Part 1
    /*
    +--------+----------+
    | Random | HMAC-MD5 |
    +--------+----------+
    |    1   |     6    |
    +--------+----------+
    */
    ret.put_u8(rng.gen());
    ret.put_slice(&hashing::sign_bytes(self.hash_kind, &part12_hmac_key, &ret[0..1])?[0..6]);

    // Part 2-2
    /*
    +-----+----------------------------+----------+
    | UID | AES-128-CBC encrypted data | HMAC-MD5 |
    +-----+----------------------------+----------+
    |  4  |             16             |     4    |
    +-----+----------------------------+----------+
    */
    ret.put_u32_le(self.processor.user_id.unwrap_or_else(|| rng.gen()));

    /*
    +-----+-----+---------------+-------------+---------------------+
    | UTC | CID | Connection ID | pack length | Random bytes length |
    +-----+---------------------+-------------+---------------------+
    |  4  |  4  |       4       |      2      |           2         |
    +-----+-----+---------------+-------------+---------------------+
    */
    let part2_enc_out = {
      let mut part2_enc = [0u8; 4 + 4 + 4 + 2 + 2];
      let utc = (unix_ts().as_secs() & 0xFFFFFFFF) as u32;
      let ids = self.processor.new_connection();
      part2_enc[0..4].copy_from_slice(&utc.to_le_bytes());
      part2_enc[4..8].copy_from_slice(&ids.0.to_le_bytes());
      part2_enc[8..12].copy_from_slice(&ids.1.to_le_bytes());
      part2_enc[12..14].copy_from_slice(&pack_len.to_le_bytes());
      part2_enc[14..16].copy_from_slice(&rnd_len.to_le_bytes());

      let cipher_kind = block::BlockCipherKind::Aes128Cbc;
      let mut part2_enc_key_raw = String::new();
      encode_config_buf(&self.user_key, base64::STANDARD, &mut part2_enc_key_raw);
      part2_enc_key_raw.push_str(match self.hash_kind {
        hashing::HashKind::Md5 => "auth_aes128_md5",
        hashing::HashKind::Sha1 => "auth_aes128_sha1",
        #[allow(unreachable_patterns)]
        _ => unimplemented!(),
      });
      let part2_enc_key = hashing::evp_bytes_to_key(
        hashing::HashKind::Md5,
        &part2_enc_key_raw.as_ref(),
        cipher_kind.key_len(),
      )?;

      let out_len = part2_enc.len() + cipher_kind.block_size();
      let mut part2_enc_out = BytesMut::with_capacity(out_len);
      unsafe {
        part2_enc_out.set_len(out_len);
      }
      let part2_enc_iv = [0u8; 16];
      let enc_n = cipher_kind
        .to_crypter(
          CrypterMode::Encrypt,
          &part2_enc_key,
          &part2_enc_iv[..],
          false,
        )?
        .update(&part2_enc[..], &mut part2_enc_out)?;
      part2_enc_out.truncate(enc_n);
      assert_eq!(enc_n, 16);
      part2_enc_out
    };
    ret.put_slice(&part2_enc_out);
    ret.put_slice(&hashing::sign_bytes(self.hash_kind, &part12_hmac_key, &ret[7..])?[0..4]);

    // Part 3
    /*
    +--------------+------------------+----------+
    | Random bytes | Origin SS stream | HMAC-MD5 |
    +--------------+------------------+----------+
    |   Variable   |     Variable     |     4    |
    +--------------+------------------+----------+
    */
    {
      let cur_len = ret.len();
      let rnd_end = cur_len + rnd_len as usize;
      unsafe {
        ret.set_len(rnd_end);
      }
      rand::rand_bytes(&mut ret[cur_len..rnd_end])?;
    }
    ret.put_slice(&buf);
    let part3_hmac = hashing::sign_bytes(self.hash_kind, &self.user_key, &ret.bytes())?;
    ret.put_slice(&part3_hmac[0..4]);
    assert_eq!(ret.len(), pack_len as usize);
    Ok(ret.freeze())
  }

  fn gen_rnd_len(&self, len: usize, full_len: usize) -> usize {
    if len > 1300 || self.last_write_len > 1300 || full_len >= 4096 {
      return 1;
    }
    let len_max = if len > 1100 {
      127
    } else if len > 900 {
      255
    } else if len > 400 {
      511
    } else {
      1023
    };
    xor_rng().gen_range(0, len_max) + 1
  }

  fn parse_rnd_len(&self, buf: &[u8]) -> usize {
    if buf[0] == 255 {
      buf[1] as usize | ((buf[2] as usize) << 8)
    } else {
      buf[0] as usize
    }
  }

  fn pack_chunk(&mut self, buf: &[u8], full_len: usize) -> Result<Bytes> {
    self.write_chunk_id += 1;

    let rand_len = self.gen_rnd_len(buf.len(), full_len);

    let mut hmac_key = BytesMut::with_capacity(self.user_key.len() + 4);
    hmac_key.put_slice(&self.user_key);
    hmac_key.put_u32_le(self.write_chunk_id);

    /*
    +------+----------+--------------+-------------------------+----------+
    | size | HMAC-MD5 | Random bytes |         Payload         | HMAC-MD5 |
    +------+----------+--------------+-------------------------+----------+
    |  2   |     2    |   Variable   | size - Random bytes - 8 |     4    |
    +------+----------+--------------+-------------------------+----------+
    */
    let pack_len = 2 + 2 + rand_len + buf.len() + 4;
    let mut ret = BytesMut::with_capacity(pack_len);

    ret.put_u16_le(pack_len as u16);
    let size_hmac = hashing::sign_bytes(self.hash_kind, &hmac_key, &ret[0..2])?;
    ret.put_slice(&size_hmac[0..2]);
    unsafe {
      ret.advance_mut(rand_len);
    }
    rand::rand_bytes(&mut ret[4..4 + rand_len])?;
    if rand_len < 128 {
      ret[4] = rand_len as u8;
    } else {
      ret[4] = 255;
      ret[5] = (rand_len & 0xFF) as u8;
      ret[6] = (rand_len >> 8) as u8;
    }
    ret.put_slice(buf);
    let pack_hmac = hashing::sign_bytes(self.hash_kind, &hmac_key, &ret.bytes())?;
    ret.put_slice(&pack_hmac[0..4]);
    assert_eq!(ret.len(), pack_len);
    Ok(ret.freeze())
  }

  fn pack_data(&mut self, mut buf: &[u8]) -> Result<VecDeque<Bytes>> {
    let full_len = buf.len();
    let mut chunks = VecDeque::with_capacity(1);
    while buf.len() > PACK_UNIT_SIZE {
      chunks.push_back(self.pack_chunk(&buf[..PACK_UNIT_SIZE], full_len)?);
      buf.advance(PACK_UNIT_SIZE);
    }
    if !buf.is_empty() {
      chunks.push_back(self.pack_chunk(&buf, full_len)?);
    }
    Ok(chunks)
  }
}

impl<RW: AsyncWrite + Unpin> AsyncWrite for AuthAes128ClientStream<RW> {
  fn poll_write(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    let me = &mut *self;

    loop {
      match &mut me.write_state {
        WriteState::PreparingData => {
          let chunks = if !me.header_sent {
            me.header_sent = true;
            let header_len = ShadowsocksClientHandshakeProcessor::header_len(buf)?;
            let divide_pos = cmp::min(buf.len(), header_len + xor_rng().gen_range(0, 31));
            let header = me
              .pack_auth_data(&buf[..divide_pos])
              .map_err(io_other_error)?;
            let mut chunks = me.pack_data(&buf[divide_pos..]).map_err(io_other_error)?;
            chunks.push_front(header);
            chunks
          } else {
            me.pack_data(buf).map_err(io_other_error)?
          };
          me.write_state = WriteState::Writing { chunks };
        }
        WriteState::Writing { chunks } => {
          let chunk = chunks.front_mut().unwrap();
          let n = ready!(Pin::new(&mut me.inner).poll_write(cx, &chunk))?;
          if n == 0 {
            return Poll::Ready(Err(eof()));
          }
          chunk.advance(n);
          if chunk.is_empty() {
            chunks.pop_front();
          }
          if chunks.is_empty() {
            me.write_state = WriteState::PreparingData;
            me.last_write_len = buf.len();
            return Poll::Ready(Ok(buf.len()));
          }
        }
      }
    }
  }
  fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.inner).poll_flush(cx)
  }
  fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.inner).poll_shutdown(cx)
  }
}

impl<RW: AsyncRead + Unpin> AsyncRead for AuthAes128ClientStream<RW> {
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    use std::convert::TryInto;

    let me = &mut *self;

    if buf.remaining() == 0 {
      return Poll::Ready(Ok(()));
    }

    loop {
      match &mut me.read_state {
        ReadState::Size => {
          if me.read_buf.len() < 7 {
            check_eof!(ready!(
              Pin::new(&mut me.inner).poll_read_buf(cx, &mut me.read_buf)
            )?);
          } else {
            let total_len = u16::from_le_bytes(me.read_buf[0..2].try_into().unwrap());
            let rnd_len = me.parse_rnd_len(&me.read_buf[4..7]) - 1;
            let payload_len = total_len as usize - (rnd_len + 1) - 8;

            me.read_buf.advance(5); // 4 + first byte of random
            me.read_state = ReadState::Random {
              rnd_len,
              payload_len,
            };
          }
        }
        ReadState::Random {
          rnd_len,
          payload_len,
        } => {
          if me.read_buf.len() < *rnd_len {
            let n = check_eof!(ready!(
              Pin::new(&mut me.inner).poll_read_buf(cx, &mut me.read_buf)
            )?);
            let consumed = cmp::min(*rnd_len, n);
            me.read_buf.advance(consumed);
            *rnd_len -= consumed;
          } else {
            me.read_buf.advance(*rnd_len);
            me.read_state = ReadState::Data {
              payload_len: *payload_len,
            };
          }
        }
        ReadState::Data { payload_len } => {
          if !me.read_buf.is_empty() && *payload_len > 0 {
            // First, flush buffer
            let n = cmp::min(buf.remaining(), cmp::min(me.read_buf.len(), *payload_len));
            buf.put_slice(&me.read_buf.split_to(n));
            *payload_len -= n;
            return Poll::Ready(Ok(()));
          } else if *payload_len > 0 {
            // Have unread data, read directly to target without overshooting
            let mut taken_buf = buf.take(*payload_len);
            let rem = taken_buf.remaining();
            ready!(Pin::new(&mut me.inner).poll_read(cx, &mut taken_buf))?;
            let n = rem - taken_buf.remaining();
            if n == 0 {
              return Poll::Ready(Ok(()));
            }

            buf.advance(n);
            *payload_len -= n;
            return Poll::Ready(Ok(()));
          } else {
            me.read_state = ReadState::Hmac;
          }
        }
        ReadState::Hmac => {
          if me.read_buf.len() >= 4 {
            me.read_buf.advance(4);
            me.read_state = ReadState::Size;
          } else {
            check_eof!(ready!(
              Pin::new(&mut me.inner).poll_read_buf(cx, &mut me.read_buf)
            )?);
          }
        }
      }
    }
  }
}
