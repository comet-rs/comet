use crate::crypto::hashing::{new_hasher, HashKind, Hasher};
use uuid::Uuid;

const ID_BYTES_LEN: usize = 16;

#[derive(Clone, Debug, Eq)]
pub struct UserId {
    uuid: Uuid,
    cmd_key: [u8; ID_BYTES_LEN],
}

impl UserId {
    pub fn new(uuid: Uuid) -> Self {
        let mut hasher = new_hasher(HashKind::Md5);
        hasher.update(&uuid.as_bytes()[..]);
        hasher.update("c48619fe-8f02-49e0-b9e9-edf763e17e21".as_bytes());

        let key = hasher.finish();
        let mut cmd_key = [0u8; ID_BYTES_LEN];
        &cmd_key[..].copy_from_slice(&key);

        Self { uuid, cmd_key }
    }

    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn cmd_key(&self) -> &[u8] {
        &self.cmd_key[..]
    }
}

impl PartialEq for UserId {
    fn eq(&self, r: &UserId) -> bool {
        self.uuid == r.uuid
    }
}

fn next_id(uuid: Uuid) -> Uuid {
    let mut hasher = new_hasher(HashKind::Md5);
    hasher.update(&uuid.as_bytes()[..]);
    hasher.update("16167dc8-16b6-4e6d-b8bb-65dd68113a81".as_bytes());

    loop {
        let new_id = hasher.clone().finish();
        if &new_id[..] != &uuid.as_bytes()[..] {
            let mut b: uuid::Bytes = Default::default();
            b[..].copy_from_slice(&new_id);
            return Uuid::from_bytes(b);
        }
        hasher.update("533eff8a-4113-4b10-b5ce-0f5d76b98cd2".as_bytes());
    }
}

pub fn new_alter_ids(primary: UserId, count: u16) -> Vec<UserId> {
    let mut ret = Vec::with_capacity(count as usize);

    let mut prev = primary.uuid();

    for _ in 0..count {
        let new_id = next_id(prev);
        ret.push(UserId::new(new_id));
        prev = new_id;
    }

    ret
}
