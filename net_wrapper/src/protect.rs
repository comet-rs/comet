use libc::c_int;

#[cfg(target_os = "android")]
fn protect(fd: c_int) -> nix::Result<()> {
  use log::info;
  use nix::sys::socket::*;
  use nix::sys::uio::IoVec;

  info!("Protecting {}", fd);
  let channel = socket(
    AddressFamily::Unix,
    SockType::Stream,
    SockFlag::SOCK_CLOEXEC,
    None,
  )?;
  connect(
    channel,
    &SockAddr::Unix(UnixAddr::new("/dev/socket/fwmarkd")?),
  )?;

  let fds = [fd];
  let cmsgs = [ControlMessage::ScmRights(&fds)];
  let iovecs = [
    IoVec::from_slice(&[
      /* command */ 3, 0, 0, 0, /* netId */ 0, 0, 0, 0, /* uid */ 0, 0, 0, 0,
      /* trafficCtrlInfo */ 0, 0, 0, 0, 255, 255, 255, 255,
    ]),
    IoVec::from_slice(&[]),
  ];

  sendmsg(channel, &iovecs, &cmsgs, MsgFlags::empty(), None)?;

  let mut buffer = [0u8; 4];
  recv(channel, &mut buffer, MsgFlags::empty())?;
  let error = i32::from_le_bytes(buffer);
  if error != 0 {
    Err(nix::Error::UnsupportedOperation)
  } else {
    Ok(())
  }
}

#[allow(unused_variables)]
#[cfg(target_os = "android")]
pub async fn protect_async(fd: c_int) -> std::io::Result<()> {
  tokio::task::spawn_blocking(move || {
    protect(fd).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, "protect failed"))
  })
  .await?
}
