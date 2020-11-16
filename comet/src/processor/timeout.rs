use crate::prelude::*;
use crate::{delegate_flush, delegate_shutdown};
use futures::Future;
use pin_project_lite::pin_project;
use std::task::Context;
use std::time::Duration;
use tokio::io::ReadBuf;
use tokio::time::Instant;
use tokio::time::Sleep;

pub fn register(plumber: &mut Plumber) {
  plumber.register("timeout", |conf| {
    let config: TimeoutConfig = from_value(conf)?;

    Ok(Box::new(TimeoutProcessor {
      idle: config.idle,
      ttfb: config.ttfb,
    }))
  });
}

#[derive(Deserialize, Debug, Clone)]
struct TimeoutConfig {
  #[serde(default)]
  idle: u64,
  #[serde(default)]
  ttfb: u64,
}

struct TimeoutProcessor {
  idle: u64,
  ttfb: u64,
}

#[async_trait]
impl Processor for TimeoutProcessor {
  async fn process(
    self: Arc<Self>,
    stream: ProxyStream,
    _conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<ProxyStream> {
    let stream = stream.into_tcp()?;
    Ok(RWPair::new(TimeoutReader::new(stream, self.ttfb, self.idle)).into())
  }
}

pin_project! {
  struct TimeoutReader<R> {
    #[pin]
    inner: R,
    timer: Option<Sleep>,
    first_packet_written: bool,
    timeout_ttfb: Option<Duration>,
    timeout_idle: Option<Duration>,
  }
}

impl<R> TimeoutReader<R> {
  fn new(inner: R, timeout_ttfb: u64, timeout_idle: u64) -> Self {
    let timeout_ttfb = if timeout_ttfb > 0 {
      Some(Duration::from_secs(timeout_ttfb))
    } else {
      None
    };
    let timeout_idle = if timeout_idle > 0 {
      Some(Duration::from_secs(timeout_idle))
    } else {
      None
    };
    Self {
      inner,
      timer: None,
      first_packet_written: false,
      timeout_ttfb,
      timeout_idle,
    }
  }
  fn reset(&mut self) {
    if self.timeout_idle.is_none() {
      return;
    }
    if let Some(timer) = &mut self.timer {
      timer.reset(Instant::now() + self.timeout_idle.unwrap());
    } else {
      self.timer = Some(tokio::time::sleep(self.timeout_idle.unwrap()));
    }
  }
  fn setup_ttfb(&mut self) {
    if let Some(timeout) = self.timeout_ttfb {
      self.timer = Some(tokio::time::sleep(timeout));
    }
  }
}

impl<R: AsyncRead + Unpin> AsyncRead for TimeoutReader<R> {
  fn poll_read(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    let me = self.as_mut().project();
    match me.inner.poll_read(cx, buf) {
      Poll::Ready(r) => {
        self.reset();
        Poll::Ready(r)
      }
      Poll::Pending => {
        if let Some(Poll::Ready(())) = self.timer.as_mut().map(|t| Pin::new(t).poll(cx)) {
          Poll::Ready(Err(io::ErrorKind::TimedOut.into()))
        } else {
          Poll::Pending
        }
      }
    }
  }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for TimeoutReader<W> {
  fn poll_write(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    let me = self.as_mut().project();
    match me.inner.poll_write(cx, buf) {
      Poll::Ready(r) => {
        if !self.first_packet_written {
          self.first_packet_written = true;
          self.setup_ttfb();
        }
        Poll::Ready(r)
      }
      Poll::Pending => Poll::Pending,
    }
  }
  delegate_flush!();
  delegate_shutdown!();
}
