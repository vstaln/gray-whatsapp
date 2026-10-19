#!/bin/sh
here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
sys="$(uname -s)"
case "$sys" in
MINGW* | MSYS* | CYGWIN*) rel="x86_64-pc-windows-msvc/whatsapp.exe" ;;
Darwin*)
    case "$(uname -m)" in
    arm64) rel="aarch64-apple-darwin/whatsapp" ;;
    *) rel="x86_64-apple-darwin/whatsapp" ;;
    esac ;;
*)
    case "$(uname -m)" in
    aarch64 | arm64) rel="aarch64-unknown-linux-musl/whatsapp" ;;
    *) rel="x86_64-unknown-linux-musl/whatsapp" ;;
    esac ;;
esac
if [ -x "$here/$rel" ]; then exec "$here/$rel" "$@"; fi
for cand in "$here/target/release/whatsapp.exe" "$here/target/release/whatsapp" "$here/target/debug/whatsapp.exe" "$here/target/debug/whatsapp"; do
    if [ -x "$cand" ]; then exec "$cand" "$@"; fi
done
echo "whatsapp binary not found in $here" >&2
exit 1
