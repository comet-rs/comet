use std::error::Error;
use std::io;

pub fn eof() -> io::Error {
    io::ErrorKind::UnexpectedEof.into()
}

pub fn crypto_error() -> io::Error {
    io_other_error("crypto error")
}

pub fn io_other_error<E: Into<Box<dyn Error + Send + Sync>>>(error: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error.into())
}

#[macro_export]
macro_rules! delegate_write {
    () => {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut self.inner).poll_write(cx, buf)
        }
    };
}

#[macro_export]
macro_rules! delegate_flush {
    () => {
        fn poll_flush(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.inner).poll_flush(cx)
        }
    };
}

#[macro_export]
macro_rules! delegate_shutdown {
    () => {
        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut self.inner).poll_shutdown(cx)
        }
    };
}

#[macro_export]
macro_rules! delegate_write_vectored {
    () => {
        fn poll_write_vectored(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
        }
    };
}

#[macro_export]
macro_rules! delegate_is_write_vectored {
    () => {
        fn is_write_vectored(&self) -> bool {
            self.inner.is_write_vectored()
        }
    };
}

#[macro_export]
macro_rules! delegate_read {
    ($type:ident) => {
        impl<R: AsyncRead + Unpin> AsyncRead for $type<R> {
            fn poll_read(
                mut self: Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
                buf: &mut tokio::io::ReadBuf<'_>,
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
            $crate::delegate_write!();
            $crate::delegate_flush!();
            $crate::delegate_shutdown!();
            $crate::delegate_write_vectored!();
            $crate::delegate_is_write_vectored!();
        }
    };
}

#[macro_export]
macro_rules! delegate_write_misc {
    () => {
        $crate::delegate_flush!();
        $crate::delegate_shutdown!();
        $crate::delegate_write_vectored!();
        $crate::delegate_is_write_vectored!();
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
