use crate::delegate_write_all;
use crate::prelude::*;
use futures::Future;
use pin_project::pin_project;
use std::task::Context;
use std::time::Duration;
use tokio::io::ReadBuf;
use tokio::time::{sleep, Instant, Sleep};

pub fn register(plumber: &mut Plumber) {
    plumber.register("timeout", |conf, _| {
        let config: TimeoutConfig = from_value(conf)?;

        Ok(Box::new(TimeoutProcessor { idle: config.idle }))
    });
}

#[derive(Deserialize, Debug, Clone)]
struct TimeoutConfig {
    #[serde(default)]
    idle: u64,
}

struct TimeoutProcessor {
    idle: u64,
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
        Ok(RWPair::new(TimeoutReader::new(stream, self.idle)).into())
    }
}
#[pin_project]
#[derive(Debug)]
struct TimeoutReader<R> {
    #[pin]
    inner: R,
    timer: Option<Pin<Box<Sleep>>>,
    timeout_idle: Option<Duration>,
}

impl<R> TimeoutReader<R> {
    fn new(inner: R, timeout_idle: u64) -> Self {
        let timeout_idle = if timeout_idle > 0 {
            Some(Duration::from_secs(timeout_idle))
        } else {
            None
        };
        Self {
            inner,
            timer: None,
            timeout_idle,
        }
    }
    fn reset(&mut self) {
        if self.timeout_idle.is_none() {
            return;
        }
        if let Some(timer) = &mut self.timer {
            timer
                .as_mut()
                .reset(Instant::now() + self.timeout_idle.unwrap());
        } else {
            self.timer = Some(Box::pin(sleep(self.timeout_idle.unwrap())));
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for TimeoutReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        let me = self.as_mut().project();
        match me.inner.poll_read(cx, buf) {
            Poll::Ready(r) => {
                self.reset();
                Poll::Ready(r)
            }
            Poll::Pending => {
                if let Some(Poll::Ready(())) = self.timer.as_mut().map(|t| t.as_mut().poll(cx)) {
                    Poll::Ready(Err(std::io::ErrorKind::TimedOut.into()))
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

delegate_write_all!(TimeoutReader);
