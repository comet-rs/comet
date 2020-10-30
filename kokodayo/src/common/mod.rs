use bytes::BufMut;
use futures::ready;
use serde::Deserialize;
use std::io;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::prelude::AsyncRead;

mod connection;
mod packet;
mod rwpair;

pub use connection::{Connection, DestAddr, UdpRequest};
pub use packet::{AsyncPacketIO, PacketIO};
pub use rwpair::RWPair;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum TransportType {
    Tcp,
    Udp,
}

pub trait MyAsyncReadExt: AsyncRead {
    /// Based on `read_buf`
    fn poll_read_buf<B: BufMut>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<usize>> {
        use crate::io::ReadBuf;
        use std::mem::MaybeUninit;

        if !buf.has_remaining_mut() {
            return Poll::Ready(Ok(0));
        }

        let n = {
            let dst = buf.bytes_mut();
            let dst = unsafe { &mut *(dst as *mut _ as *mut [MaybeUninit<u8>]) };
            let mut buf = ReadBuf::uninit(dst);
            let ptr = buf.filled().as_ptr();
            ready!(self.poll_read(cx, &mut buf)?);

            // Ensure the pointer does not change from under us
            assert_eq!(ptr, buf.filled().as_ptr());
            buf.filled().len()
        };

        // Safety: This is guaranteed to be the number of initialized (and read)
        // bytes due to the invariants provided by `ReadBuf::filled`.
        unsafe {
            buf.advance_mut(n);
        }

        Poll::Ready(Ok(n))
    }
}

impl<T: AsyncRead> MyAsyncReadExt for T {}