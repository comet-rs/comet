use crate::sniffer::SniffStatus;
use std::convert::TryInto;
use std::str;

mod consts {
    pub const TYPE_HANDSHAKE: u8 = 0x16;
    pub const MAJOR_VERSION_3: u8 = 0x03;
}

const FAIL_NOT_HANDSHAKE: SniffStatus = SniffStatus::Fail("Not handshake message");

pub fn read_client_hello(mut b: &[u8]) -> SniffStatus {
    if b.len() < 42 {
        return FAIL_NOT_HANDSHAKE;
    }

    let session_id_len = b[38] as usize;
    if session_id_len > 32 || b.len() < 39 + session_id_len {
        return FAIL_NOT_HANDSHAKE;
    }
    b = b.split_at(39 + session_id_len).1;
    if b.len() < 2 {
        return FAIL_NOT_HANDSHAKE;
    }

    let cipher_suite_len = (b[0] as usize) << 8 | (b[1] as usize);
    if cipher_suite_len % 2 == 1 {
        return FAIL_NOT_HANDSHAKE;
    }
    if b.len() < 2 + cipher_suite_len {
        return FAIL_NOT_HANDSHAKE;
    }

    b = b.split_at(2 + cipher_suite_len).1;
    if b.len() < 1 {
        return FAIL_NOT_HANDSHAKE;
    }

    let compression_methods_len = b[0] as usize;
    if b.len() < 1 + compression_methods_len {
        return FAIL_NOT_HANDSHAKE;
    }

    b = b.split_at(1 + compression_methods_len).1;
    if b.len() < 2 {
        return FAIL_NOT_HANDSHAKE;
    }
    let extensions_len = (b[0] as usize) << 8 | (b[1] as usize);
    b = b.split_at(2).1;
    if extensions_len != b.len() {
        return FAIL_NOT_HANDSHAKE;
    }

    while b.len() > 0 {
        if b.len() < 4 {
            return FAIL_NOT_HANDSHAKE;
        }
        let extension_id = (b[0] as usize) << 8 | (b[1] as usize);
        let extension_len = (b[2] as usize) << 8 | (b[3] as usize);
        b = b.split_at(4).1;

        if b.len() < extension_len {
            return FAIL_NOT_HANDSHAKE;
        }

        if extension_id == 0x00 {
            // SNI
            let mut d = b.split_at(extension_len).0;
            if d.len() != extension_len {
                return FAIL_NOT_HANDSHAKE;
            }

            let names_len = (d[0] as usize) << 8 | (d[1] as usize);
            d = d.split_at(2).1;
            if d.len() != names_len {
                return FAIL_NOT_HANDSHAKE;
            }
            while d.len() > 0 {
                if d.len() < 4 {
                    return FAIL_NOT_HANDSHAKE;
                }
                let name_type = d[0];
                let name_len = (d[1] as usize) << 8 | (d[2] as usize);
                d = d.split_at(3).1;
                if d.len() < name_len {
                    return FAIL_NOT_HANDSHAKE;
                }
                if name_type == 0 {
                    if let Ok(server_name) = str::from_utf8(&d[0..name_len]) {
                        if server_name.ends_with(".") {
                            return FAIL_NOT_HANDSHAKE;
                        }
                        return SniffStatus::Success(server_name.to_owned());
                    } else {
                        return FAIL_NOT_HANDSHAKE;
                    }
                }
            }
        }
    }

    FAIL_NOT_HANDSHAKE
}

pub fn sniff(b: &[u8]) -> SniffStatus {
    if b.len() < 5 {
        return SniffStatus::NoClue;
    }
    if b[0] != consts::TYPE_HANDSHAKE {
        return SniffStatus::Fail("Not handshake message");
    }
    if b[1] != consts::MAJOR_VERSION_3 {
        return SniffStatus::Fail("Invalid TLS version");
    }
    let header_len = u16::from_be_bytes((&b[3..5]).try_into().unwrap());
    if (5 + header_len) as usize > b.len() {
        return SniffStatus::NoClue;
    }
    return read_client_hello(&b[5..(header_len + 5) as usize]);
}
