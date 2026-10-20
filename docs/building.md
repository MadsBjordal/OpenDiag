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

### Android signing key (needed for updates)

Android installs a new version over the old one only if both are signed with
the same key. The release workflow signs every APK with the project's key,
which lives in GitHub secrets, never in the repository. Create it **once**:

1. On Windows, in PowerShell (Android Studio includes `keytool`):
   ```powershell
   & "C:\Program Files\Android\Android Studio\jbr\bin\keytool.exe" -genkeypair -v -keystore $HOME\opendiag.jks -keyalg RSA -keysize 2048 -validity 10000 -alias opendiag
   ```
   (Without Android Studio: any Java JDK has `keytool`, e.g. Temurin 17.)
   Choose a strong password and answer the name questions (they appear in the certificate).
2. Turn the key into text for GitHub:
   ```powershell
   [Convert]::ToBase64String([IO.File]::ReadAllBytes("$HOME\opendiag.jks")) | Set-Clipboard
   ```
3. GitHub → the OpenDiag repository → **Settings → Secrets and variables → Actions → New repository secret**, four times:
   - `ANDROID_KEYSTORE_BASE64`: paste (Ctrl+V) from step 2
   - `ANDROID_KEYSTORE_PASSWORD`: the password
   - `ANDROID_KEY_ALIAS`: `opendiag`
   - `ANDROID_KEY_PASSWORD`: the password (same as above unless you chose another)
4. **Keep `opendiag.jks` and the password safe** (e.g. a password manager and a
   USB stick). If the key is lost, no later build can update installed apps:
   everyone would have to uninstall and reinstall. Never commit it.

Without these secrets the release workflow stops with a message saying so.

## Updates

Installed builds check GitHub for a newer release when they start (Settings →
Updates; can be turned off). The release workflow stamps each build with its
release name (`OPENDIAG_RELEASE`) and, on Android, a rising version code.
Windows downloads the installer, checks it against GitHub's SHA-256 and runs it;
Android opens the download, and Android's installer updates the app. Development
builds (no `OPENDIAG_RELEASE`) are never offered updates.

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
