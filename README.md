# SysInfo Dashboard - Rust Native Edition

A Rust-native Android device diagnostics app. The UI is rendered directly to a pixel buffer via the NDK's `ANativeWindow` — no Java UI toolkit, no XML layouts.

## Architecture

- **`src/main.rs`** — NativeActivity entry point, event loop, rendering pipeline
- **`src/android_api.rs`** — JNI bridges to Android system APIs
- **`src/render.rs`** — Pixel-buffer text renderer with embedded bitmap font

## Building

```bash
# Install prerequisites
rustup target add aarch64-linux-android
cargo install cargo-apk

# Build APK
cargo apk build --release

# Output: target/release/apk/sysinfo-dashboard.apk
```

## APK Download

See [Releases](https://github.com/hitduress67/DeviceInfo-Rust/releases) for the latest Rust-native APK.
