use std::error::Error;
use std::io;

pub fn eof() -> io::Error {
  io::Error::new(io::ErrorKind::UnexpectedEof, "early eof")
}

pub fn crypto_error() -> io::Error {
  io::Error::new(io::ErrorKind::Other, "crypto error")
}

pub fn io_other_error<E: Into<Box<dyn Error + Send + Sync>>>(error: E) -> io::Error {
  io::Error::new(io::ErrorKind::Other, error.into())
}

#[macro_export]
macro_rules! delegate_write {
  () => {
    fn poll_write(
      mut self: std::pin::Pin<&mut Self>,
      cx: &mut Context<'_>,
      buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
      Pin::new(&mut self.inner).poll_write(cx, buf)
    }
  };
}

#[macro_export]
macro_rules! delegate_flush {
  () => {
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
      Pin::new(&mut self.inner).poll_flush(cx)
    }
  };
}

#[macro_export]
macro_rules! delegate_shutdown {
  () => {
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
      Pin::new(&mut self.inner).poll_shutdown(cx)
    }
  };
}

#[macro_export]
macro_rules! delegate_read {
  ($type:ident) => {
    impl<R: AsyncRead + Unpin> AsyncRead for $type<R> {
      fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
      ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
      }
    }
  };
}

#[macro_export]
macro_rules! delegate_write_all {
  ($type:ident) => {
    impl<W: AsyncWrite + Unpin> AsyncWrite for $type<W> {
      crate::delegate_write!();
      crate::delegate_flush!();
      crate::delegate_shutdown!();
    }
  };
}

#[macro_export]
macro_rules! check_eof {
  ($s:expr) => {{
    let n = $s;
    if n == 0 {
      return Poll::Ready(Ok(()));
    }
    n
  }};
}