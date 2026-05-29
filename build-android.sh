#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

# ── Prerequisites check ──
for cmd in rustup npm npx java; do
    command -v "$cmd" >/dev/null || { echo "Missing: $cmd"; exit 1; }
done

if [ -z "${ANDROID_HOME:-}" ] && [ -z "${ANDROID_SDK_ROOT:-}" ]; then
    # Common default paths
    if [ -d "$HOME/Library/Android/sdk" ]; then
        export ANDROID_HOME="$HOME/Library/Android/sdk"
    elif [ -d "$HOME/Android/Sdk" ]; then
        export ANDROID_HOME="$HOME/Android/Sdk"
    else
        echo "Error: ANDROID_HOME not set and SDK not found in default locations."
        exit 1
    fi
fi
export ANDROID_SDK_ROOT="${ANDROID_HOME:-$ANDROID_SDK_ROOT}"
export NDK_HOME="${NDK_HOME:-$ANDROID_SDK_ROOT/ndk/$(ls "$ANDROID_SDK_ROOT/ndk/" 2>/dev/null | sort -V | tail -1)}"

if [ ! -d "$NDK_HOME" ]; then
    echo "Error: NDK not found at $NDK_HOME"
    echo "Install via: sdkmanager --install 'ndk;27.0.12077973'"
    exit 1
fi

echo "SDK: $ANDROID_SDK_ROOT"
echo "NDK: $NDK_HOME"

# ── Rust Android targets ──
TARGETS=(aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android)
for t in "${TARGETS[@]}"; do
    rustup target add "$t" 2>/dev/null || true
done

echo "=== 1. npm install ==="
npm ci

# ── frpc binary for Android (optional, requires Go) ──
FRPC_JNI_DIR="src-tauri/gen/android/app/src/main/jniLibs"
if command -v go &>/dev/null; then
    echo "=== 1.5 Build frpc for Android ==="
    FRPC_VERSION="${FRPC_VERSION:-0.61.1}"
    declare -A FRPC_ARCH_MAP=(
        ["aarch64-linux-android"]="arm64-v8a"
        ["armv7-linux-androideabi"]="armeabi-v7a"
        ["x86_64-linux-android"]="x86_64"
        ["i686-linux-android"]="x86"
    )
    for rust_target in "${!FRPC_ARCH_MAP[@]}"; do
        android_abi="${FRPC_ARCH_MAP[$rust_target]}"
        echo "  Building frpc for $android_abi..."
        mkdir -p "$FRPC_JNI_DIR/$android_abi"

        # Map Rust target to Go GOARCH
        case "$rust_target" in
            aarch64-*)   goarch="arm64" ;;
            armv7-*)     goarch="arm" ;;
            x86_64-*)    goarch="amd64" ;;
            i686-*)      goarch="386" ;;
            *)           echo "  Unknown target: $rust_target"; continue ;;
        esac

        # Use NDK clang as CC for CGO
        export CGO_ENABLED=1
        export GOOS=android
        export GOARCH="$goarch"
        export CC="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/${rust_target}24-clang"

        go build -buildmode=pie \
            -o "$FRPC_JNI_DIR/$android_abi/libfrpc.so" \
            "github.com/fatedier/frp/cmd/frpc@v${FRPC_VERSION}" 2>/dev/null \
            || echo "  WARNING: frpc build failed for $android_abi (skipping)"
    done
    unset CGO_ENABLED GOOS GOARCH CC
else
    echo "=== 1.5 Skipping frpc build (Go not found) ==="
    echo "  Install Go to bundle frpc, or place pre-built libfrpc.so in $FRPC_JNI_DIR/{abi}/"
fi

echo "=== 2. Build Tauri Android (APK) ==="
npx tauri android build

echo "=== Done ==="
echo ""

# Find output APKs
APK_DIR="src-tauri/gen/android/app/build/outputs/apk"
if [ -d "$APK_DIR" ]; then
    echo "APKs:"
    find "$APK_DIR" -name "*.apk" -exec ls -lh {} \;
fi

# Find output AABs
AAB_DIR="src-tauri/gen/android/app/build/outputs/bundle"
if [ -d "$AAB_DIR" ]; then
    echo ""
    echo "AABs:"
    find "$AAB_DIR" -name "*.aab" -exec ls -lh {} \;
fi
