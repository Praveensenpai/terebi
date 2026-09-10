#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

# 📺 Terebi (テレビ) All-in-One Fast Installer
# One-liner: curl -fsSL https://raw.githubusercontent.com/Praveensenpai/terebi/main/install.sh | bash

REPO="Praveensenpai/terebi"
BIN_DIR="${HOME}/.local/bin"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/terebi"
SYSTEMD_USER_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

TMP_DIR="$(mktemp -d -t terebi_install.XXXXXXXXXX)"
cleanup() {
    local exit_code=$?
    rm -rf "${TMP_DIR}"
    exit "${exit_code}"
}
trap cleanup EXIT INT TERM HUP

log_info()  { printf "\033[1;34mℹ\033[0m %s\n" "$*" >&2; }
log_ok()    { printf "\033[1;32m✔\033[0m %s\n" "$*" >&2; }
log_warn()  { printf "\033[1;33m⚠\033[0m %s\n" "$*" >&2; }
log_error() { printf "\033[1;31m✖\033[0m %s\n" "$*" >&2; }

detect_target() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    if [[ "${os}" != "linux" ]]; then
        log_error "Unsupported OS: ${os}. Terebi currently supports Linux."
        exit 1
    fi

    case "${arch}" in
        x86_64|amd64) TARGET_ARCH="x86_64" ;;
        aarch64|arm64) TARGET_ARCH="aarch64" ;;
        *) log_error "Unsupported architecture: ${arch}"; exit 1 ;;
    esac

    TARGET="${TARGET_ARCH}-unknown-linux-gnu"
}

download_release() {
    detect_target
    log_info "Fetching latest release for ${TARGET}..."

    local api_url="https://api.github.com/repos/${REPO}/releases/latest"
    local download_url
    download_url="$(curl -fsSL "${api_url}" | grep "browser_download_url.*${TARGET}.tar.gz" | cut -d : -f 2,3 | tr -d ' "')"

    if [[ -z "${download_url}" ]]; then
        log_error "Could not find a pre-compiled release archive for ${TARGET}."
        if command -v cargo >/dev/null 2>&1; then
            log_info "Falling back to building from source with cargo..."
            build_from_source
            return 0
        fi
        exit 1
    fi

    log_info "Downloading ${download_url}..."
    curl -fSL "${download_url}" -o "${TMP_DIR}/terebi.tar.gz"

    log_info "Extracting release archive..."
    tar -xzf "${TMP_DIR}/terebi.tar.gz" -C "${TMP_DIR}"

    local extracted_bin
    extracted_bin="$(find "${TMP_DIR}" -name terebi -type f | head -n 1)"

    if [[ -z "${extracted_bin}" || ! -f "${extracted_bin}" ]]; then
        log_error "Failed to locate extracted terebi binary."
        exit 1
    fi

    mkdir -p "${BIN_DIR}"
    install -Dm 755 "${extracted_bin}" "${BIN_DIR}/terebi"
    log_ok "Installed terebi to ${BIN_DIR}/terebi"
}

build_from_source() {
    if ! command -v cargo >/dev/null 2>&1; then
        log_error "Rust toolchain ('cargo') is required to build from source."
        exit 1
    fi
    log_info "Building release binary locally with cargo..."
    cargo build --release
    mkdir -p "${BIN_DIR}"
    install -Dm 755 "target/release/terebi" "${BIN_DIR}/terebi"
    log_ok "Installed binary to ${BIN_DIR}/terebi"
}

check_adb() {
    if ! command -v adb >/dev/null 2>&1; then
        log_warn "'adb' not found. Please install adb (sudo apt install -y adb) to connect to your TV."
    fi
}

configure_and_connect() {
    mkdir -p "${CONFIG_DIR}"
    if [[ ! -f "${CONFIG_DIR}/config.json" ]]; then
        log_info "No configuration found. Starting interactive setup..."
        if [[ -e /dev/tty ]]; then
            "${BIN_DIR}/terebi" setup </dev/tty >/dev/tty 2>&1 || true
        else
            cat << 'EOF' > "${CONFIG_DIR}/config.json.example"
{
  "bot_token": "YOUR_TELEGRAM_BOT_TOKEN",
  "chat_id": "YOUR_TELEGRAM_CHAT_ID",
  "tv_ip": "auto",
  "tv_port": 5555,
  "friendly_name": "Living Room TV"
}
EOF
            log_info "Created template config at ${CONFIG_DIR}/config.json.example"
            log_info "Run 'terebi setup' to configure interactively."
        fi
    else
        log_ok "Found existing configuration at ${CONFIG_DIR}/config.json"
    fi

    if [[ -f "${CONFIG_DIR}/config.json" ]]; then
        log_info "Connecting to TV with dynamic auto-discovery..."
        "${BIN_DIR}/terebi" connect || log_warn "TV connection test pending authorization on TV screen."
    fi
}

setup_and_start_service() {
    if ! command -v systemctl >/dev/null 2>&1; then
        log_warn "systemd not detected. Skipping user service setup."
        return 0
    fi

    log_info "Installing systemd user service..."
    mkdir -p "${SYSTEMD_USER_DIR}"

    cat << EOF > "${SYSTEMD_USER_DIR}/terebi.service"
[Unit]
Description=Terebi (テレビ) Smart TV Telegram Controller & Watchdog
After=network.target

[Service]
Type=simple
ExecStart=${BIN_DIR}/terebi daemon
Restart=on-failure
RestartSec=10s

[Install]
WantedBy=default.target
EOF

    systemctl --user daemon-reload
    log_ok "Installed systemd user service at ${SYSTEMD_USER_DIR}/terebi.service"

    log_info "Starting terebi background service..."
    systemctl --user enable --now terebi.service || true
    log_ok "Terebi watchdog service is now active in the background!"
}

show_status() {
    if [[ -f "${CONFIG_DIR}/config.json" ]]; then
        printf "\n"
        "${BIN_DIR}/terebi" status || true
    fi
}

main() {
    printf "\n  \033[1m📺 \033[36mTerebi (テレビ) All-in-One Installer\033[0m\n"
    printf "  %s\n\n" "─────────────────────────────────────────"

    check_adb

    local mode="download"
    for arg in "$@"; do
        if [[ "${arg}" == "--build" ]]; then
            mode="build"
        fi
    done

    if [[ "${mode}" == "build" && -f "Cargo.toml" ]]; then
        build_from_source
    else
        download_release
    fi

    configure_and_connect
    setup_and_start_service
    show_status

    printf "\n  \033[1;32m✔\033[0m \033[1mInstallation and startup complete!\033[0m\n"
    printf "  Manage service: '\033[1;36msystemctl --user status terebi.service\033[0m'\n"
    printf "  Send Telegram commands: '\033[1;36m/status\033[0m', '\033[1;36m/screen\033[0m', '\033[1;36m/remote\033[0m'\n\n"
}

main "$@"
