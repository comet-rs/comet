use crate::utils::unix_ts;
use std::net::IpAddr;

use lz_fnv::{Fnv1a, FnvHasher};
use rand::{thread_rng, Rng};

use super::SecurityType;
use crate::{
    crypto::{
        hashing::{hash_bytes, new_hasher, HashKind, Hasher},
        random::xor_rng,
        stream::{StreamCipherKind, StreamCrypter},
        CrypterMode,
    },
    prelude::*,
};
use anyhow::bail;

pub struct ClientSession {
    request_key: Bytes,
    request_iv: Bytes,
    response_key: Bytes,
    response_iv: Bytes,
    auth_v: u8,
    security: SecurityType,
}

impl ClientSession {
    pub fn new(sec: SecurityType) -> Self {
        let mut rng = thread_rng();

        let req_key: [u8; 16] = rng.gen();
        let req_iv: [u8; 16] = rng.gen();

        let res_key = hash_bytes(HashKind::Md5, &req_key[..]);
        let res_iv = hash_bytes(HashKind::Md5, &req_iv[..]);

        Self {
            request_key: Bytes::copy_from_slice(&req_key[..]),
            request_iv: Bytes::copy_from_slice(&req_key[..]),
            response_key: res_key,
            response_iv: res_iv,
            auth_v: rng.gen(),
            security: sec,
        }
    }

    pub fn encode_request_header(&self, conn: &Connection, cmd_key: &[u8]) -> Result<BytesMut> {
        let mut rng = xor_rng();
        /*
        | 1 字节 | 16 字节 | 16 字节 | 1 字节 | 1 字节 | 4 位 | 4 位 | 1 字节 | 1 字节 | 2 字节 | 1 字节 | N 字节 | P 字节 | 4 字节 |
        |:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
        | 版本号 Ver | 数据加密 IV | 数据加密 Key | 响应认证 V | 选项 Opt | 余量 P | 加密方式 Sec | 保留 | 指令 Cmd | 端口 Port | 地址类型 T | 地址 A | 随机值 | 校验 F |
        */
        let mut ret =
            BytesMut::with_capacity(1 + 16 + 16 + 1 + 1 + 1 /* 4 + 4 bits */ + 1 + 1 + 2 + 1);
        ret.put_u8(1); // Ver
        ret.put_slice(&self.request_iv[..16]); // IV
        ret.put_slice(&self.request_key[..16]); // Key
        ret.put_u8(self.auth_v); // V
        ret.put_u8(0x01 | 0x04 | 0x08); // Opt = ChunkStream | ChunkMasking | GlobalPadding

        let padding_len = rng.gen_range(0..16);
        let sec = match self.security {
            SecurityType::Aes128Gcm => 0x02,
            SecurityType::Chacha20Poly1305 => 0x03,
            SecurityType::Auto => bail!("Auto should not be here"),
        };
        let cmd = match conn.typ {
            TransportType::Tcp => 0x01,
            TransportType::Udp => 0x02,
        };
        ret.put_slice(&[(padding_len << 4) | sec, 0, cmd]);
        ret.put_u16(conn.dest_addr.port_or_error()?);

        match (conn.dest_addr.domain.as_ref(), conn.dest_addr.ip) {
            (Some(d), _) => {
                ret.put_u8(0x02);
                ret.put_u8(d.len() as u8);
                ret.put_slice(d.as_bytes());
            }
            (_, Some(IpAddr::V4(ip))) => {
                ret.put_u8(0x01);
                ret.put_slice(&ip.octets()[..]);
            }
            (_, Some(IpAddr::V6(ip))) => {
                ret.put_u8(0x03);
                ret.put_slice(&ip.octets()[..]);
            }
            (None, None) => bail!("Invalid dest addr"),
        }

        if padding_len > 0 {
            ret.reserve(padding_len as usize);
            for _ in 0..padding_len {
                ret.put_u8(rng.gen());
            }
        }

        let mut hasher = Fnv1a::<u32>::new();
        hasher.write(&ret);
        ret.put_u32(hasher.finish());

        let timestamp = unix_ts().as_secs().to_be_bytes();
        let mut iv_hasher = new_hasher(HashKind::Md5);
        for _ in 0..4 {
            iv_hasher.update(&timestamp[..]);
        }
        let cmd_iv = iv_hasher.finish();

        let mut crypter =
            StreamCipherKind::Aes128Cfb.to_crypter(CrypterMode::Encrypt, &cmd_key, &cmd_iv)?;

        let _ = crypter.update(&mut ret);

        Ok(ret)
    }
}
