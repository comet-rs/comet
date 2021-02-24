use crate::{crypto::hashing::sign_bytes, utils::unix_ts};
use std::{convert::TryInto, net::IpAddr};

use lz_fnv::{Fnv1a, FnvHasher};
use rand::{thread_rng, Rng};

use super::{alter_id::UserId, SecurityType};
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

#[derive(Debug, Clone)]
pub struct ClientSession {
    auth_info: [u8; 16],
    pub cmd_key: [u8; 16],
    pub cmd_iv: [u8; 16],
    pub request_key: [u8; 16],
    pub request_iv: [u8; 16],
    pub response_key: [u8; 16],
    pub response_iv: [u8; 16],
    auth_v: u8,
}

impl ClientSession {
    pub fn new(user: &UserId) -> Self {
        let mut rng = thread_rng();

        let req_key: [u8; 16] = rng.gen();
        let req_iv: [u8; 16] = rng.gen();

        let res_key = hash_bytes(HashKind::Md5, &req_key[..])
            .as_ref()
            .try_into()
            .unwrap();
        let res_iv = hash_bytes(HashKind::Md5, &req_iv[..])
            .as_ref()
            .try_into()
            .unwrap();

        let timestamp = unix_ts().as_secs().to_be_bytes();

        let cmd_iv = {
            let mut iv_hasher = new_hasher(HashKind::Md5);
            for _ in 0..4 {
                iv_hasher.update(&timestamp[..]);
            }
            iv_hasher.finish().as_ref().try_into().unwrap()
        };

        let auth_info = sign_bytes(HashKind::Md5, &user.uuid().as_bytes()[..], &timestamp[..])
            .as_ref()
            .try_into()
            .unwrap();

        Self {
            auth_info,
            cmd_key: user.cmd_key(),
            cmd_iv,
            request_key: req_key,
            request_iv: req_iv,
            response_key: res_key,
            response_iv: res_iv,
            auth_v: rng.gen(),
        }
    }

    pub fn encode_request_header(&self, sec: SecurityType, conn: &Connection) -> Result<BytesMut> {
        let mut rng = xor_rng();
        /*
        | 1 字节 | 16 字节 | 16 字节 | 1 字节 | 1 字节 | 4 位 | 4 位 | 1 字节 | 1 字节 | 2 字节 | 1 字节 | N 字节 | P 字节 | 4 字节 |
        |:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
        | 版本号 Ver | 数据加密 IV | 数据加密 Key | 响应认证 V | 选项 Opt | 余量 P | 加密方式 Sec | 保留 | 指令 Cmd | 端口 Port | 地址类型 T | 地址 A | 随机值 | 校验 F |
        */
        let mut ret =
            BytesMut::with_capacity(16 + 1 + 16 + 16 + 1 + 1 + 1 /* 4 + 4 bits */ + 1 + 1 + 2 + 1);
        ret.put_slice(&self.auth_info); // Auth
        ret.put_u8(1); // Ver
        ret.put_slice(&self.request_iv); // IV
        ret.put_slice(&self.request_key); // Key
        ret.put_u8(self.auth_v); // V
        ret.put_u8(0x01 | 0x04 | 0x08); // Opt = ChunkStream | ChunkMasking | GlobalPadding

        let padding_len = rng.gen_range(0..16);
        let sec = match sec {
            SecurityType::Aes128Gcm => 0x03,
            SecurityType::Chacha20Poly1305 => 0x04,
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
        hasher.write(&ret[16..]);
        ret.put_u32(hasher.finish());

        let mut crypter = StreamCipherKind::Aes128Cfb.to_crypter(
            CrypterMode::Encrypt,
            &self.cmd_key,
            &self.cmd_iv,
        )?;

        let _ = crypter.update(&mut ret[16..]);

        Ok(ret)
    }

    pub fn decode_response_header(&self, buf: &[u8]) -> Result<()> {
        if buf.len() < 4 {
            bail!("Buffer too short");
        }
        let mut buf: [u8; 4] = buf[0..4].try_into()?;

        let mut crypter = StreamCipherKind::Aes128Cfb.to_crypter(
            CrypterMode::Decrypt,
            &self.response_key,
            &self.response_iv,
        )?;
        crypter.update(&mut buf)?;

        if buf[0] != self.auth_v {
            bail!("Authentication mismatch");
        }

        if buf[3] != 0 {
            bail!("Dynamic port is not supported");
        }

        Ok(())
    }
}
