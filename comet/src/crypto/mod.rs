pub mod aead;
pub mod block;
pub mod hashing;
pub mod random;
pub mod stream;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrypterMode {
    Encrypt,
    Decrypt,
}
