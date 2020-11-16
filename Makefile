.PHONY: android-debug android-release windows run
android-debug:
  RUSTFLAGS=-g cargo ndk --platform 29 --target aarch64-linux-android build -p comet-android --lib

android-release:
	cargo ndk --platform 29 --target aarch64-linux-android build -p comet-android --lib --release

windows:
	RUSTFLAGS="-Ctarget-cpu=sandybridge -Ctarget-feature=+aes,+sse2,+sse4.1,+ssse3" cargo build --target x86_64-pc-windows-gnu -p comet-bin --release

run:
	cargo run -p comet-bin

release:
	RUSTFLAGS="-Ctarget-cpu=sandybridge -Ctarget-feature=+aes,+sse2,+sse4.1,+ssse3" cargo build -p comet-bin --release