RUSTFLAGS=-g cargo ndk --platform 29 --target aarch64-linux-android build -p kokodayo-android --lib
# cargo build -p app --lib --target aarch64-linux-android --release