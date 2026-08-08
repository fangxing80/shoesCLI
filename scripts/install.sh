#!/usr/bin/env sh
#
# shoes installer.
#
# Downloads a prebuilt `shoes` release binary from GitHub Releases, installs it
# to a bin directory, and (on Linux with systemd) sets up a system service.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/fangxing80/shoesCLI/master/scripts/install.sh | sh
#
# Environment overrides:
#   SHOES_VERSION   Release tag to install (default: latest).
#   SHOES_BIN_DIR   Install directory for the binary (default: /usr/local/bin).
#   SHOES_CONFIG_DIR  Config directory (default: /etc/shoes).
#   SHOES_NO_SERVICE  If set to 1, skip systemd service setup.
#   SHOES_USE_MUSL  If set to 1, prefer the statically-linked musl build on Linux.
#
set -eu

REPO="fangxing80/shoesCLI"
BIN_NAME="shoes"
BIN_DIR="${SHOES_BIN_DIR:-/usr/local/bin}"
CONFIG_DIR="${SHOES_CONFIG_DIR:-/etc/shoes}"
CONFIG_FILE="${CONFIG_DIR}/config.shoes.yaml"
SERVICE_NAME="shoes"

# --- output helpers ---------------------------------------------------------

if [ -t 1 ]; then
    C_INFO='\033[1;32m'
    C_WARN='\033[1;33m'
    C_ERR='\033[1;31m'
    C_DIM='\033[2m'
    C_OFF='\033[0m'
else
    C_INFO='' C_WARN='' C_ERR='' C_DIM='' C_OFF=''
fi

info() { printf "${C_INFO}==>${C_OFF} %s\n" "$1"; }
warn() { printf "${C_WARN}warning:${C_OFF} %s\n" "$1" >&2; }
err() { printf "${C_ERR}error:${C_OFF} %s\n" "$1" >&2; }
die() {
    err "$1"
    exit 1
}

# --- prerequisite checks ----------------------------------------------------

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        die "required command not found: $1"
    fi
}

# Pick a downloader.
DOWNLOADER=""
if command -v curl >/dev/null 2>&1; then
    DOWNLOADER="curl"
elif command -v wget >/dev/null 2>&1; then
    DOWNLOADER="wget"
else
    die "need either curl or wget to download releases"
fi

need_cmd tar
need_cmd uname

# fetch <url> <output-path>
fetch() {
    _url="$1"
    _out="$2"
    if [ "$DOWNLOADER" = "curl" ]; then
        curl -fsSL "$_url" -o "$_out"
    else
        wget -qO "$_out" "$_url"
    fi
}

# fetch_stdout <url>
fetch_stdout() {
    _url="$1"
    if [ "$DOWNLOADER" = "curl" ]; then
        curl -fsSL "$_url"
    else
        wget -qO- "$_url"
    fi
}

# --- platform detection -----------------------------------------------------

# The gnu release requires glibc >= 2.38. Return success (0) when the system
# glibc is older than that or cannot be determined, so the caller falls back to
# the static musl build. Returns failure (non-zero) when glibc is new enough.
glibc_too_old() {
    _ver=""
    # Prefer `getconf`, then `ldd --version`, to read the glibc version.
    if command -v getconf >/dev/null 2>&1; then
        _ver="$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}')"
    fi
    if [ -z "$_ver" ] && command -v ldd >/dev/null 2>&1; then
        _ver="$(ldd --version 2>/dev/null | head -n1 | grep -oE '[0-9]+\.[0-9]+' | head -n1)"
    fi

    # No detectable glibc (e.g. musl-only system) -> prefer musl.
    [ -z "$_ver" ] && return 0

    _major="${_ver%%.*}"
    _minor="${_ver#*.}"
    _minor="${_minor%%.*}"
    # Non-numeric parse -> be safe and prefer musl.
    case "$_major$_minor" in
    *[!0-9]*) return 0 ;;
    esac

    # Too old when major < 2, or major == 2 and minor < 38.
    if [ "$_major" -lt 2 ]; then
        return 0
    fi
    if [ "$_major" -eq 2 ] && [ "$_minor" -lt 38 ]; then
        return 0
    fi
    return 1
}

detect_target() {
    _os="$(uname -s)"
    _arch="$(uname -m)"

    case "$_arch" in
    x86_64 | amd64) _rust_arch="x86_64" ;;
    aarch64 | arm64) _rust_arch="aarch64" ;;
    *) die "unsupported architecture: $_arch" ;;
    esac

    case "$_os" in
    Linux)
        # Choose between the glibc (gnu) and static (musl) build.
        #
        # The gnu release is built on a modern CI image and links against a
        # recent glibc (>= 2.38). On older distros that symbol version is
        # missing, so the binary fails at startup with "GLIBC_2.xx not found".
        # musl is fully static and runs anywhere, so we fall back to it when
        # requested, on Alpine, or when the system glibc is too old / absent.
        if [ "${SHOES_USE_MUSL:-0}" = "1" ]; then
            _libc="musl"
        elif [ -f /etc/alpine-release ]; then
            _libc="musl"
        elif glibc_too_old; then
            info "System glibc is older than 2.38; using the static musl build."
            _libc="musl"
        else
            _libc="gnu"
        fi
        TARGET="${_rust_arch}-unknown-linux-${_libc}"
        PLATFORM="linux"
        ;;
    Darwin)
        TARGET="${_rust_arch}-apple-darwin"
        PLATFORM="macos"
        ;;
    *)
        die "unsupported operating system: $_os"
        ;;
    esac
}

# --- release resolution -----------------------------------------------------

resolve_version() {
    if [ -n "${SHOES_VERSION:-}" ]; then
        VERSION="$SHOES_VERSION"
        return
    fi
    info "Resolving latest release..."
    _api="https://api.github.com/repos/${REPO}/releases/latest"
    # Extract the tag_name from the JSON without requiring jq.
    VERSION="$(fetch_stdout "$_api" | grep -m1 '"tag_name"' | sed -E 's/.*"tag_name"[^"]*"([^"]+)".*/\1/')"
    if [ -z "$VERSION" ]; then
        die "could not resolve the latest release tag. Set SHOES_VERSION explicitly."
    fi
}

# --- download & install -----------------------------------------------------

# Whether privilege escalation is needed for the chosen install/config dirs.
# Set once in main() after directories are known.
NEED_ROOT=0

# sudo wrapper: escalate only when the current user cannot write the targets.
as_root() {
    if [ "$(id -u)" -eq 0 ] || [ "$NEED_ROOT" -eq 0 ]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        die "this step needs root privileges but sudo is not available: $*"
    fi
}

# Decide whether root is required to write to BIN_DIR (and CONFIG_DIR on Linux).
# A directory is considered writable if it exists and is writable, or if its
# nearest existing ancestor is writable (so it can be created).
dir_writable() {
    _d="$1"
    while [ -n "$_d" ] && [ ! -e "$_d" ]; do
        _parent="$(dirname "$_d")"
        [ "$_parent" = "$_d" ] && break
        _d="$_parent"
    done
    [ -w "$_d" ]
}

determine_privilege() {
    if [ "$(id -u)" -eq 0 ]; then
        NEED_ROOT=0
        return
    fi
    if dir_writable "$BIN_DIR"; then
        NEED_ROOT=0
    else
        NEED_ROOT=1
    fi
}

download_and_install() {
    _tarball="${BIN_NAME}CLI-${TARGET}.tar.gz"
    _url="https://github.com/${REPO}/releases/download/${VERSION}/${_tarball}"

    _tmp="$(mktemp -d 2>/dev/null || mktemp -d -t shoes)"
    # Clean up the temp dir on exit.
    trap 'rm -rf "$_tmp"' EXIT INT TERM

    info "Downloading ${_tarball} (${VERSION})..."
    if ! fetch "$_url" "${_tmp}/${_tarball}"; then
        err "download failed: $_url"
        die "no prebuilt binary for target '${TARGET}'. Check https://github.com/${REPO}/releases"
    fi

    info "Extracting..."
    tar -xzf "${_tmp}/${_tarball}" -C "$_tmp"

    if [ ! -f "${_tmp}/${BIN_NAME}" ]; then
        die "extracted archive did not contain a '${BIN_NAME}' binary"
    fi

    chmod +x "${_tmp}/${BIN_NAME}"

    info "Installing to ${BIN_DIR}/${BIN_NAME}..."
    as_root mkdir -p "$BIN_DIR"
    as_root install -m 0755 "${_tmp}/${BIN_NAME}" "${BIN_DIR}/${BIN_NAME}"

    info "Installed: $("${BIN_DIR}/${BIN_NAME}" --version 2>/dev/null || echo "${BIN_NAME} (version check unavailable)")"
}

# --- systemd service --------------------------------------------------------

has_systemd() {
    [ "$PLATFORM" = "linux" ] && command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]
}

setup_service() {
    info "Setting up systemd service..."

    as_root mkdir -p "$CONFIG_DIR"

    # Create a starter config only if none exists, so re-running the installer
    # never clobbers a user's configuration.
    if [ ! -f "$CONFIG_FILE" ]; then
        info "Writing a starter config to ${CONFIG_FILE}"
        _uuid="$("${BIN_DIR}/${BIN_NAME}" generate-vless-user-id 2>/dev/null | grep -Eo '[0-9a-f-]{36}' | head -n1 || true)"
        [ -n "$_uuid" ] || _uuid="REPLACE-WITH-YOUR-UUID"
        _starter="$(mktemp)"
        cat >"$_starter" <<EOF
# Starter shoes config generated by the installer.
# Edit this file, then run:  systemctl restart ${SERVICE_NAME}
# For an interactive wizard:  shoes menu
- address: 0.0.0.0:1080
  protocol:
    type: socks
    udp_enabled: true
EOF
        as_root install -m 0644 "$_starter" "$CONFIG_FILE"
        rm -f "$_starter"
        # _uuid is retained for user reference in the log below.
        printf "${C_DIM}A VLESS-compatible UUID you can use: %s${C_OFF}\n" "$_uuid"
    else
        info "Keeping existing config at ${CONFIG_FILE}"
    fi

    _unit="$(mktemp)"
    cat >"$_unit" <<EOF
[Unit]
Description=shoes multi-protocol proxy server
Documentation=https://github.com/${REPO}
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=${BIN_DIR}/${BIN_NAME} ${CONFIG_FILE}
Restart=on-failure
RestartSec=3
# Hardening
DynamicUser=yes
AmbientCapabilities=CAP_NET_BIND_SERVICE
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
ReadOnlyPaths=${CONFIG_DIR}
LimitNOFILE=1048576

[Install]
WantedBy=multi-user.target
EOF
    as_root install -m 0644 "$_unit" "/etc/systemd/system/${SERVICE_NAME}.service"
    rm -f "$_unit"

    as_root systemctl daemon-reload
    as_root systemctl enable "${SERVICE_NAME}.service" >/dev/null 2>&1 || true

    info "Service installed. Manage it with:"
    printf "${C_DIM}  systemctl start %s${C_OFF}\n" "$SERVICE_NAME"
    printf "${C_DIM}  systemctl status %s${C_OFF}\n" "$SERVICE_NAME"
    printf "${C_DIM}  journalctl -u %s -f${C_OFF}\n" "$SERVICE_NAME"
    warn "Edit ${CONFIG_FILE} before starting, or run 'shoes menu' to generate one."
}

# --- main -------------------------------------------------------------------

main() {
    detect_target
    info "Detected platform: ${TARGET}"
    determine_privilege
    resolve_version
    download_and_install

    if [ "${SHOES_NO_SERVICE:-0}" = "1" ]; then
        info "SHOES_NO_SERVICE=1 set; skipping service setup."
    elif has_systemd; then
        setup_service
    else
        info "No systemd detected; skipping service setup."
        printf "${C_DIM}Run the server with:  %s /path/to/config.shoes.yaml${C_OFF}\n" "$BIN_NAME"
        printf "${C_DIM}Or generate a config: %s menu${C_OFF}\n" "$BIN_NAME"
    fi

    info "Done."
}

main "$@"
