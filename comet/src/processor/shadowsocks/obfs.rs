use crate::check_eof;
use crate::crypto::rand;
use crate::prelude::*;
use crate::utils::io::eof;
use crate::{delegate_flush, delegate_read, delegate_shutdown, delegate_write_all};
use futures::ready;
use lazy_static::lazy_static;
use std::cmp;
use std::io;
use std::task::Context;
use tokio::io::ReadBuf;
use xorshift::Rng;

pub fn register(plumber: &mut Plumber) {
  plumber.register("ssr_obfs_client", |conf| {
    Ok(Box::new(ClientProcessor {
      config: from_value(conf)?,
    }))
  });
}

lazy_static! {
  static ref USER_AGENTS: Vec<&'static str> = vec![
    "Mozilla/5.0 (Windows NT 6.3; WOW64; rv:40.0) Gecko/20100101 Firefox/40.0",
    "Mozilla/5.0 (Windows NT 6.3; WOW64; rv:40.0) Gecko/20100101 Firefox/44.0",
    "Mozilla/5.0 (Windows NT 6.1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/41.0.2228.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/535.11 (KHTML, like Gecko) Ubuntu/11.10 Chromium/27.0.1453.93 Chrome/27.0.1453.93 Safari/537.36",
    "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:35.0) Gecko/20100101 Firefox/35.0",
    "Mozilla/5.0 (compatible; WOW64; MSIE 10.0; Windows NT 6.2)",
    "Mozilla/5.0 (Windows; U; Windows NT 6.1; en-US) AppleWebKit/533.20.25 (KHTML, like Gecko) Version/5.0.4 Safari/533.20.27",
    "Mozilla/4.0 (compatible; MSIE 7.0; Windows NT 6.3; Trident/7.0; .NET4.0E; .NET4.0C)",
    "Mozilla/5.0 (Windows NT 6.3; Trident/7.0; rv:11.0) like Gecko",
    "Mozilla/5.0 (Linux; Android 4.4; Nexus 5 Build/BuildID) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/30.0.0.0 Mobile Safari/537.36",
    "Mozilla/5.0 (iPad; CPU OS 5_0 like Mac OS X) AppleWebKit/534.46 (KHTML, like Gecko) Version/5.1 Mobile/9A334 Safari/7534.48.3",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 5_0 like Mac OS X) AppleWebKit/534.46 (KHTML, like Gecko) Version/5.1 Mobile/9A334 Safari/7534.48.3"
  ];
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "obfs")]
pub enum ClientConfig {
  #[serde(rename = "http_simple")]
  HttpSimple {
    #[serde(default)]
    hosts: Vec<SmolStr>,
    #[serde(default)]
    headers: HashMap<SmolStr, SmolStr>,
    port: u16,
  },
}

#[derive(Debug)]
pub struct ClientProcessor {
  config: ClientConfig,
}

#[async_trait]
impl Processor for ClientProcessor {
  async fn process(
    self: Arc<Self>,
    stream: RWPair,
    conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<RWPair> {
    let stream = match &self.config {
      ClientConfig::HttpSimple {
        hosts,
        headers,
        port,
      } => {
        let host = if hosts.is_empty() {
          if conn.dest_addr.domain.is_some() {
            format!("{}", conn.dest_addr.domain.as_ref().unwrap())
          } else {
            format!("{}", conn.dest_addr.ip.as_ref().unwrap())
          }
        } else {
          rand::xor_rng().choose(&hosts).unwrap().to_string()
        };

        let mut header_buf = BytesMut::new();
        header_buf.put_slice(format!("Host: {}", host).as_bytes());
        if *port != 80 {
          header_buf.put_slice(format!(":{}", port).as_bytes());
        }
        header_buf.put_slice(b"\r\n");

        if headers.is_empty() {
          header_buf.put_slice(b"User-Agent: ");
          header_buf.put_slice(rand::xor_rng().choose(&USER_AGENTS).unwrap().as_bytes());
          header_buf.put_slice(b"\r\nAccept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\nAccept-Language: en-US,en;q=0.8\r\nAccept-Encoding: gzip, deflate\r\nDNT: 1\r\nConnection: keep-alive\r\n");
        } else {
          for (name, value) in headers {
            header_buf.put_slice(format!("{}: {}\r\n", name, value).as_bytes());
          }
        }
        header_buf.put_slice(b"\r\n");
        SimpleHttpWriter::new(stream, ObfsHttpMethod::Get, header_buf)
      }
    };
    Ok(RWPair::new(StripHttpHeaderStream::new(stream)))
  }
}

enum StripState {
  Stripping,
  WritingBuf,
  Done,
}

struct StripHttpHeaderStream<RW> {
  inner: RW,
  state: StripState,
  buf: Option<BytesMut>,
}

impl<RW> StripHttpHeaderStream<RW> {
  fn new(inner: RW) -> Self {
    Self {
      inner,
      state: StripState::Stripping,
      buf: Some(BytesMut::with_capacity(512)),
    }
  }
}

impl<R: AsyncRead + Unpin> AsyncRead for StripHttpHeaderStream<R> {
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    let me = &mut *self;
    if buf.remaining() == 0 {
      return Poll::Ready(Ok(()));
    }

    loop {
      match &mut me.state {
        StripState::Stripping => {
          let me_buf = me.buf.as_mut().unwrap();
          if me_buf.remaining_mut() == 0 {
            me_buf.reserve(512);
          }
          check_eof!(ready!(Pin::new(&mut me.inner).poll_read_buf(cx, me_buf))?);
          for i in 0..me_buf.len() - 4 {
            if &me_buf[i..i + 4] == b"\r\n\r\n" {
              me_buf.advance(i + 4);
              me.state = StripState::WritingBuf;
              break;
            }
          }
        }
        StripState::WritingBuf => {
          let me_buf = me.buf.as_mut().unwrap();
          let n = std::cmp::min(me_buf.len(), buf.remaining());
          buf.put_slice(&me_buf[..n]);
          me_buf.advance(n);
          if me_buf.is_empty() {
            me.buf.take();
            me.state = StripState::Done;
          }
          return Poll::Ready(Ok(()));
        }
        StripState::Done => {
          return Pin::new(&mut me.inner).poll_read(cx, buf);
        }
      }
    }
  }
}
delegate_write_all!(StripHttpHeaderStream);

#[derive(Deserialize, Debug, Clone)]
enum ObfsHttpMethod {
  Get,
  Post,
}

enum HttpWriterState {
  Prepare(ObfsHttpMethod, BytesMut),
  Writing(usize, BytesMut),
  Done,
}

struct SimpleHttpWriter<RW> {
  inner: RW,
  state: HttpWriterState,
}

impl<RW> SimpleHttpWriter<RW> {
  fn new(inner: RW, method: ObfsHttpMethod, header_buf: BytesMut) -> Self {
    Self {
      inner,
      state: HttpWriterState::Prepare(method, header_buf),
    }
  }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for SimpleHttpWriter<W> {
  fn poll_write(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    let me = &mut *self;
    loop {
      match &mut me.state {
        HttpWriterState::Prepare(method, header_buf) => {
          let mut full_header_buf = BytesMut::from(match method {
            ObfsHttpMethod::Get => "GET /",
            ObfsHttpMethod::Post => "POST /",
          });
          let encode_len = cmp::min(buf.len(), 30 + 16 + rand::xor_rng().gen_range(0, 64));
          full_header_buf.reserve(encode_len * 3);
          for byte in &buf[..encode_len] {
            let s = format!("{:x}", byte);
            full_header_buf.put_u8(b'%');
            if s.len() == 1 {
              full_header_buf.put_u8(b'0');
            }
            full_header_buf.put_slice(s.as_bytes());
          }
          full_header_buf.put_slice(b" HTTP/1.1\r\n");
          full_header_buf.extend_from_slice(&header_buf);
          me.state = HttpWriterState::Writing(encode_len, full_header_buf);
        }
        HttpWriterState::Writing(encode_len, full_header_buf) => {
          let n = ready!(Pin::new(&mut me.inner).poll_write(cx, full_header_buf))?;
          if n == 0 {
            return Poll::Ready(Err(eof()));
          }
          full_header_buf.advance(n);
          if full_header_buf.is_empty() {
            let encode_len = *encode_len;
            me.state = HttpWriterState::Done;
            return Poll::Ready(Ok(encode_len));
          }
        }
        HttpWriterState::Done => {
          return Pin::new(&mut me.inner).poll_write(cx, buf);
        }
      }
    }
  }

  delegate_flush!();
  delegate_shutdown!();
}

delegate_read!(SimpleHttpWriter);
