# Building and installing

## Windows (app and installer)

One-time setup:

1. Install **Rust**: download and run `rustup-init.exe` from https://rustup.rs (accept the defaults).
   It asks for the **Visual Studio C++ Build Tools**: install them with the
   "Desktop development with C++" workload.
2. Install **Node.js 22 LTS** from https://nodejs.org, then in a terminal: `npm install -g pnpm`.
3. Install **Git** from https://git-scm.com and clone the project:
   `git clone https://github.com/MadsBjordal/OpenDiag.git C:\OpenDiag`
   (a short path without special characters avoids Android build problems later).
4. WebView2 is part of Windows 11; on Windows 10 install the "Evergreen" runtime if the app shows a blank window.

Run and build (in `C:\OpenDiag`):

```powershell
pnpm install
pnpm tauri dev                          # starts the app while developing
pnpm tauri build                        # installer: target\release\bundle\nsis\OpenDiag_0.1.0_x64-setup.exe
cargo test                              # all engine tests
```

USB adapters: if no COM port appears, install the driver for the adapter's USB
chip (FTDI, CH340 or CP210x). Bluetooth adapters: pair them in Windows Settings
first; choose the outgoing COM port in OpenDiag.

## Android (APK)

One-time setup (on top of the Windows/Linux steps above):

1. Install **Android Studio**, then in *SDK Manager* install: Android SDK Platform 34,
   Build-Tools 34, Platform-Tools, and *NDK (Side by side)* 27.
2. Set environment variables (Windows: *Edit the system environment variables*):
   `ANDROID_HOME` = `%LOCALAPPDATA%\Android\Sdk`,
   `NDK_HOME` = `%LOCALAPPDATA%\Android\Sdk\ndk\27.2.12479018` (your NDK version),
   `JAVA_HOME` = Android Studio's JBR folder (e.g. `C:\Program Files\Android\Android Studio\jbr`).
3. `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android`

Build:

```powershell
pnpm install
pnpm tauri android build --apk --target aarch64     # release APK (needs signing, see below)
pnpm tauri android build --apk --target aarch64 --debug   # debug APK, installable directly
```

The APK is under `apps\desktop\src-tauri\gen\android\app\build\outputs\apk\`.
Install it by copying it to the phone and opening it (allow "install unknown apps"),
or with `adb install path\to.apk` with USB debugging on.

Release signing: create a keystore once
(`keytool -genkey -v -keystore opendiag.jks -keyalg RSA -keysize 2048 -validity 10000 -alias opendiag`),
keep it and its passwords out of the repository, and configure it as described in the
Tauri documentation ("Android code signing").

## Linux / macOS

```sh
# Debian/Ubuntu
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev librsvg2-dev libudev-dev
pnpm install && pnpm tauri dev
```

USB serial on Linux: add yourself to the `dialout` group.

## Development without hardware

```sh
pnpm build:ui
cargo run -p opendiag-cli -- serve --ui apps/ui/dist      # http://127.0.0.1:7878
# or, with live reload:
cargo run -p opendiag-cli -- serve    # bridge on :7878
pnpm dev                              # UI on http://localhost:5173
```
