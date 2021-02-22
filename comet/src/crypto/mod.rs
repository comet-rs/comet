pub mod aead;
pub mod block;
pub mod hashing;
pub mod random;
pub mod stream;

pub enum CrypterMode {
    Encrypt,
    Decrypt,
}
