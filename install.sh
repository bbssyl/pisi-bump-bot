#!/bin/sh
set -eu

REPO_URL="https://github.com/bbssyl/pisi-bump-bot"
BINARY_NAME="pisi-bump-tui"

if ! command -v cargo >/dev/null 2>&1; then
    echo "Hata: cargo bulunamadı." >&2
    echo "pisi-bump-tui, Rust araç zincirini (cargo) gerektirir." >&2
    echo "Kurulum için: https://rustup.rs" >&2
    exit 1
fi

echo "pisi-bump-tui kuruluyor (cargo install --git ${REPO_URL} ${BINARY_NAME})..."
cargo install --git "${REPO_URL}" "${BINARY_NAME}"

install_root="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-${HOME}/.cargo}}"
binary_path="${install_root}/bin/${BINARY_NAME}"

echo ""
echo "Kurulum tamamlandı: ${binary_path}"
echo "Bu dizin PATH içinde değilse, kabuk yapılandırma dosyanıza şunu ekleyin:"
echo "  export PATH=\"${install_root}/bin:\$PATH\""
