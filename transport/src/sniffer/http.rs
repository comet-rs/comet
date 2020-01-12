use crate::sniffer::SniffStatus;
use std::str;

const HTTP_METHODS: [&'static str; 7] =
    ["GET", "POST", "HEAD", "PUT", "DELETE", "OPTIONS", "CONNECT"];

fn match_http_method(buf: &[u8]) -> Option<bool> {
    for method in HTTP_METHODS.iter() {
        if buf.len() >= method.len() {
            if let Ok(s) = str::from_utf8(&buf[0..method.len()]) {
                if method.eq_ignore_ascii_case(s) {
                    return Some(true);
                }
            }
        }
        if buf.len() < method.len() {
            return None;
        }
    }
    Some(false)
}

pub fn sniff(b: &[u8]) -> SniffStatus {
    if let Some(matched) = match_http_method(b) {
        if !matched {
            return SniffStatus::Fail("Not HTTP method");
        }
    } else {
        return SniffStatus::NoClue;
    }

    let mut headers_iter = b.split(|b| *b == b'\n').skip(1).peekable();

    while let Some(header) = headers_iter.next() {
        if header.len() == 0 {
            break;
        }
        let parts: Vec<&[u8]> = header.splitn(2, |b| *b == b':').collect();
        if parts.len() != 2 {
            continue;
        }
        if let Ok(key) = str::from_utf8(parts[0]) {
            if key.eq_ignore_ascii_case("host") {
                if headers_iter.peek().is_none() {
                    return SniffStatus::NoClue; // Current line not finished
                }
                if let Ok(value) = str::from_utf8(parts[1]) {
                    let trimmed = value.trim().to_owned();
                    return SniffStatus::Success(trimmed);
                } else {
                    return SniffStatus::Fail("Failed to decode header value");
                }
            }
        } else {
            return SniffStatus::Fail("Failed to decode header key");
        }
    }
    SniffStatus::NoClue
}
