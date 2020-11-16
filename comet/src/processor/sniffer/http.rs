use super::SniffStatus;
use httparse::{Request, EMPTY_HEADER};

pub fn sniff(b: &[u8]) -> super::SniffStatus {
    let mut headers = [EMPTY_HEADER; 32];
    let mut req = Request::new(&mut headers);
    match req.parse(b) {
        Ok(status) => {
            for header in req.headers {
                if header.name.eq_ignore_ascii_case("host") {
                    return SniffStatus::Success(String::from_utf8_lossy(header.value).to_string());
                }
            }
            if status.is_complete() {
                SniffStatus::Fail("No Host")
            } else {
                SniffStatus::NoClue
            }
        }
        Err(_) => SniffStatus::Fail("Not HTTP"),
    }
}
