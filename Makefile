.PHONY: android-debug android-release windows
android-debug:
  RUSTFLAGS=-g cargo ndk --platform 29 --target aarch64-linux-android build -p comet-android --lib

android-release:
	cargo ndk --platform 29 --target aarch64-linux-android build -p comet-android --lib --release

windows:
	cargo build --target x86_64-pc-windows-gnu -p comet-bin --release