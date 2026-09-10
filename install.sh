#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

# 📺 Terebi (テレビ) Installer & Service Setup

BIN_DIR="${HOME}/.local/bin"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/terebi"
SYSTEMD_USER_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

log_info()  { printf "\033[1;34mℹ\033[0m %s\n" "$*" >&2; }
log_ok()    { printf "\033[1;32m✔\033[0m %s\n" "$*" >&2; }
log_warn()  { printf "\033[1;33m⚠\033[0m %s\n" "$*" >&2; }
log_error() { printf "\033[1;31m✖\033[0m %s\n" "$*" >&2; }

check_dependencies() {
    log_info "Checking dependencies..."
    if ! command -v cargo >/dev/null 2>&1; then
        log_error "Rust toolchain ('cargo') is required to build terebi from source."
        exit 1
    fi
    if ! command -v adb >/dev/null 2>&1; then
        log_warn "'adb' not found. Please install adb (e.g. sudo apt install -y adb) to connect to your TV."
    fi
    log_ok "Dependencies verified."
}

build_binary() {
    log_info "Building release binary..."
    cargo build --release
    mkdir -p "${BIN_DIR}"
    install -Dm 755 "target/release/terebi" "${BIN_DIR}/terebi"
    log_ok "Installed binary to ${BIN_DIR}/terebi"
}

setup_config() {
    mkdir -p "${CONFIG_DIR}"
    if [[ ! -f "${CONFIG_DIR}/config.json" ]]; then
        log_info "No configuration found at ${CONFIG_DIR}/config.json."
        if [[ -f "configs/terebi.example.json" ]]; then
            cp "configs/terebi.example.json" "${CONFIG_DIR}/config.json.example"
            log_info "Copied template to ${CONFIG_DIR}/config.json.example"
        fi
        log_info "Run '${BIN_DIR}/terebi setup' to configure your bot token and TV IP interactively."
    else
        log_ok "Found existing configuration at ${CONFIG_DIR}/config.json"
    fi
}

setup_systemd_service() {
    if ! command -v systemctl >/dev/null 2>&1; then
        log_warn "systemd not detected. Skipping user service setup."
        return 0
    fi

    log_info "Installing systemd user service..."
    mkdir -p "${SYSTEMD_USER_DIR}"

    cat <<EOF > "${SYSTEMD_USER_DIR}/terebi.service"
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
    log_info "To enable and start Terebi as a background service:"
    printf "    systemctl --user enable --now terebi.service\n"
}

main() {
    printf "\n  \033[1m📺 \033[36mTerebi (テレビ) Installer\033[0m\n"
    printf "  %s\n\n" "─────────────────────────────────────────"

    check_dependencies
    build_binary
    setup_config
    setup_systemd_service

    printf "\n  \033[1;32m✔\033[0m \033[1mInstallation complete!\033[0m\n"
    printf "  Quickstart: run '\033[1;36mterebi setup\033[0m' or '\033[1;36mterebi status\033[0m'\n\n"
}

main "$@"
