use crate::prelude::*;
use std::cmp::min;

#[derive(Debug, Copy, Clone)]
pub enum HashKind {
    Md5,
    Sha1,
}

impl HashKind {
    pub fn output_len(&self) -> usize {
        match self {
            HashKind::Md5 => 16,
            HashKind::Sha1 => 20,
        }
    }
}

pub trait Hasher {
    fn update(&mut self, data: &[u8]) -> Result<()>;
    fn finish(&mut self) -> Result<Bytes>;
}

pub trait Signer {
    fn update(&mut self, data: &[u8]) -> Result<()>;
    fn finish(&mut self) -> Result<Bytes>;
}

pub fn new_hasher(kind: HashKind) -> Result<Box<dyn Hasher>> {
    #[cfg(target_os = "windows")]
    {
        use self::windows::WindowsHasher;
        let hasher = Box::new(WindowsHasher::new(kind)?);
        Ok(hasher)
    }
    #[cfg(not(target_os = "windows"))]
    {
        use self::openssl::OpensslHasher;
        let hasher = Box::new(OpensslHasher::new(kind.into())?);
        Ok(hasher)
    }
}

pub fn hash_bytes(kind: HashKind, input: &[u8]) -> Result<Bytes> {
    let mut hasher = new_hasher(kind)?;
    hasher.update(input)?;
    Ok(hasher.finish()?)
}

pub fn new_signer(kind: HashKind, key: &[u8]) -> Result<Box<dyn Signer>> {
    #[cfg(target_os = "windows")]
    {
        use self::windows::WindowsSigner;
        let hasher = Box::new(WindowsSigner::new(kind, key)?);
        Ok(hasher)
    }
    #[cfg(not(target_os = "windows"))]
    {
        use self::openssl::OpensslSigner;
        let signer = Box::new(OpensslSigner::new(kind, key)?);
        Ok(signer)
    }
}

pub fn sign_bytes(kind: HashKind, key: &[u8], input: &[u8]) -> Result<Bytes> {
    let mut signer = new_signer(kind, key)?;
    signer.update(input)?;
    let ret = signer.finish()?;
    Ok(ret)
}

pub fn evp_bytes_to_key(kind: HashKind, input: &[u8], len: usize) -> Result<Bytes> {
    let mut buf = BytesMut::with_capacity(len);
    let mut last_hash: Option<Bytes> = None;

    while buf.len() < len {
        let mut hasher = new_hasher(kind)?;
        if let Some(last_hash) = last_hash {
            hasher.update(&last_hash)?;
        }
        hasher.update(input)?;
        let hash = hasher.finish()?;
        let write_len = min(hash.len(), len - buf.len());
        buf.put_slice(&hash[0..write_len]);
        last_hash = Some(hash);
    }

    Ok(buf.freeze())
}

#[cfg(test)]
mod test {
    use super::{evp_bytes_to_key, HashKind};

    #[test]
    fn bytes_to_key() {
        let key = evp_bytes_to_key(HashKind::Md5, b"abc", 32).unwrap();
        assert_eq!(
      &b"\x90\x01P\x98<\xd2O\xb0\xd6\x96?}(\xe1\x7fr\xea\x0b1\xe1\x08z\"\xbcS\x94\xa6cnn\xd3K"[..],
      &key[..]
    );
    }
}

#[cfg(not(target_os = "windows"))]
mod openssl {
    use super::{HashKind, Hasher, Signer};
    use crate::prelude::*;

    pub use openssl::hash::Hasher as OpensslHasher;
    use openssl::hash::MessageDigest;
    use openssl::pkey::{PKey, Private};
    use openssl::sign::Signer as _OpensslSigner;

    impl Into<MessageDigest> for HashKind {
        fn into(self) -> MessageDigest {
            match self {
                HashKind::Md5 => MessageDigest::md5(),
                HashKind::Sha1 => MessageDigest::sha1(),
            }
        }
    }

    impl Hasher for OpensslHasher {
        fn update(&mut self, data: &[u8]) -> Result<()> {
            Ok(OpensslHasher::update(self, data)?)
        }
        fn finish(&mut self) -> Result<Bytes> {
            let digest = OpensslHasher::finish(self)?;
            Ok(Bytes::copy_from_slice(digest.as_ref()))
        }
    }

    pub struct OpensslSigner {
        signer: _OpensslSigner<'static>,
        _pkey: Box<PKey<Private>>,
    }

    impl OpensslSigner {
        pub fn new(kind: HashKind, key: &[u8]) -> Result<Self> {
            let pkey = Box::into_raw(Box::new(PKey::hmac(key)?));
            let signer = _OpensslSigner::new(kind.into(), unsafe { pkey.as_ref().unwrap() })?;
            let s = Self {
                _pkey: unsafe { Box::from_raw(pkey) },
                signer,
            };
            Ok(s)
        }
    }

    impl Signer for OpensslSigner {
        fn update(&mut self, data: &[u8]) -> Result<()> {
            Ok(self.signer.update(data)?)
        }
        fn finish(&mut self) -> Result<Bytes> {
            let max_len = self.signer.len()?;
            let mut buf = BytesMut::with_capacity(max_len);
            unsafe {
                buf.set_len(max_len);
            }
            let n = self.signer.sign(&mut buf)?;
            buf.truncate(n);
            Ok(buf.freeze())
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{HashKind, Hasher, Signer};
    use crate::prelude::*;
    use win_crypto_ng::hash::digest_trait::{Md5, Sha1, WinDigest};

    use hmac::{Hmac, Mac, NewMac};
    use win_crypto_ng::hash::{Hash, HashAlgorithm, HashAlgorithmId};

    pub struct WindowsHasher {
        inner: Option<Hash>,
    }

    impl WindowsHasher {
        pub fn new(kind: HashKind) -> Result<Self> {
            let algo = HashAlgorithm::open(match kind {
                HashKind::Md5 => HashAlgorithmId::Md5,
                HashKind::Sha1 => HashAlgorithmId::Sha1,
            })?;
            Ok(Self {
                inner: Some(algo.new_hash()?),
            })
        }
    }

    impl Hasher for WindowsHasher {
        fn update(&mut self, data: &[u8]) -> Result<()> {
            Ok(self.inner.as_mut().unwrap().hash(data)?)
        }
        fn finish(&mut self) -> Result<Bytes> {
            let inner = self.inner.take().unwrap();
            let buffer = inner.finish()?;
            Ok(Bytes::copy_from_slice(buffer.as_slice()))
        }
    }

    pub struct WindowsSigner {
        inner: Option<WindowsSignerInner>,
    }

    impl WindowsSigner {
        pub fn new(kind: HashKind, key: &[u8]) -> Result<Self> {
            let ret = match kind {
                HashKind::Md5 => WindowsSignerInner::Md5(Hmac::new_varkey(key).unwrap()),
                HashKind::Sha1 => WindowsSignerInner::Sha1(Hmac::new_varkey(key).unwrap()),
            };

            Ok(Self { inner: Some(ret) })
        }
    }

    pub enum WindowsSignerInner {
        Md5(Hmac<WinDigest<Md5>>),
        Sha1(Hmac<WinDigest<Sha1>>),
    }

    impl WindowsSignerInner {
        fn update(&mut self, data: &[u8]) {
            match self {
                Self::Md5(m) => m.update(data),
                Self::Sha1(m) => m.update(data),
            }
        }

        fn finish(self) -> Bytes {
            match self {
                Self::Md5(m) => Bytes::copy_from_slice(&m.finalize().into_bytes()),
                Self::Sha1(m) => Bytes::copy_from_slice(&m.finalize().into_bytes()),
            }
        }
    }

    impl Signer for WindowsSigner {
        fn update(&mut self, data: &[u8]) -> Result<()> {
            self.inner.as_mut().unwrap().update(data);
            Ok(())
        }
        fn finish(&mut self) -> Result<Bytes> {
            let inner = self.inner.take().unwrap();
            Ok(inner.finish())
        }
    }
}
