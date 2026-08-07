# shoes

shoes is a high-performance multi-protocol proxy server written in Rust.

## Supported Protocols

### Proxy Protocols
- **HTTP/HTTPS**
- **SOCKS5** (with UDP ASSOCIATE)
- **Mixed** (auto-detect HTTP/SOCKS5)
- **VMess AEAD**
- **VLESS** (with fallback support)
- **Shadowsocks**
- **Trojan**
- **Snell v3**
- **Hysteria2**
- **TUIC v5**
- **AnyTLS**
- **NaiveProxy**
- **H2MUX** (supported with VMess, VLESS, Trojan, Shadowsocks, Snell)

### Transport Protocols
All server protocols plus:
- **SagerNet UDP over TCP** (for Shadowsocks, SOCKS5, AnyTLS, NaiveProxy)
- **ShadowTLS v3**
- **TLS**
- **WebSocket** (Shadowsocks SIP003)
- **XTLS Reality**
- **XTLS Vision** (for VLESS)

### TUN/VPN Mode
- **TUN device support** - Layer 3 VPN for transparent proxying
- Supported platforms: Linux, Android, iOS

### Supported Ciphers
- **VMess**: `aes-128-gcm`, `chacha20-poly1305`, `none`
- **Shadowsocks**: `aes-128-gcm`, `aes-256-gcm`, `chacha20-ietf-poly1305`, `2022-blake3-aes-128-gcm`, `2022-blake3-aes-256-gcm`, `2022-blake3-chacha20-ietf-poly1305`
- **Snell v3**: `aes-128-gcm`, `aes-256-gcm`, `chacha20-ietf-poly1305`

## Features

- **Multi-transport**: TCP or QUIC for all protocols
- **TLS with SNI routing**: Route by Server Name Indication
- **Upstream proxy chaining**: Multi-hop chains with load balancing
- **Rule-based routing**: Route by IP/CIDR or hostname masks
- **Named PEM certificates**: Define once, reference everywhere
- **TLS fingerprint authentication**: Certificate pinning for TLS/QUIC
- **Hot reloading**: Apply config changes without restart
- **Unix socket support**: Bind to Unix domain sockets
- **Interactive wizard**: Menu-driven config generation with `shoes menu`
- **One-line installer**: `curl | sh` install with optional systemd service on Linux

For advanced access control (IP allowlist/blocklists), see [tobaru](https://github.com/cfal/tobaru).

## Installation

### One-line install (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/fangxing80/shoesCLI/master/scripts/install.sh | sh
```

The script detects your OS/architecture, downloads the matching release binary to
`/usr/local/bin`, and on Linux with systemd installs and enables a `shoes` service.

Environment overrides:

| Variable | Default | Purpose |
|---|---|---|
| `SHOES_VERSION` | latest | Release tag to install |
| `SHOES_BIN_DIR` | `/usr/local/bin` | Binary install directory |
| `SHOES_CONFIG_DIR` | `/etc/shoes` | Config directory (systemd) |
| `SHOES_NO_SERVICE` | `0` | Set to `1` to skip systemd setup |
| `SHOES_USE_MUSL` | `0` | Set to `1` to prefer the static musl build on Linux |

### Prebuilt binaries

Precompiled binaries for x86_64 and Apple aarch64 are available on [Github Releases](https://github.com/cfal/shoes/releases).

### From source

```bash
cargo install shoes
```

## Usage

```
shoes [OPTIONS] <config.yaml> [config.yaml...]

OPTIONS:
    -t, --threads NUM    Set the number of worker threads (default: CPU count)
    -l, --log-file PATH  Log to file (repeatable; "-" means stderr; default: stderr)
    -d, --dry-run        Parse the config and exit
    --no-reload          Disable automatic config reloading on file changes
    -V, --version        Print version information and exit

COMMANDS:
    menu                                           Launch the interactive config wizard (alias: wizard)
    generate-reality-keypair                       Generate a new Reality X25519 keypair
    generate-shadowsocks-2022-password <cipher>    Generate a Shadowsocks password
    generate-vless-user-id                         Generate a random VLESS/VMess user ID (UUID v4)
```

With no config argument, `shoes` loads `config.shoes.yaml` from the current directory.

### Interactive wizard

Run `shoes menu` (alias `shoes wizard`) for a menu-driven interface that generates a
server config without hand-writing YAML:

```
================================
   shoes  interactive  menu
================================
  1  Generate a server config (wizard)
  2  Generate a REALITY keypair
  3  Generate a VLESS/VMess UUID
  4  Generate a Shadowsocks-2022 password
  5  Validate a config file (dry-run)
  0  Exit
```

The config generator supports VLESS+REALITY+Vision, VMess, Shadowsocks, Trojan,
Hysteria2, TUIC v5, AnyTLS, NaiveProxy, Snell v3, ShadowTLS v3, SOCKS5, HTTP, and
Mixed (HTTP+SOCKS5). It auto-generates keys/UUIDs/short IDs and Shadowsocks-2022 keys,
fills sensible defaults, validates the result, and writes it to `config.shoes.yaml`
(or a path you choose).

For VLESS+REALITY, VMess, Shadowsocks, Trojan, Hysteria2, TUIC, and NaiveProxy the
wizard also prints a client **import link** after saving (it prompts for your server's
public IP/domain, since the bind address is usually `0.0.0.0`). Paste the link into
v2rayN / Shadowrocket / Clash-family clients to import the node. Example:

```
-- Client import link --
Server public IP or domain (for the share link) [www.microsoft.com]: 203.0.113.10
Node label [shoes]: my-node

  import URL (VLESS-REALITY)
  vless://<uuid>@203.0.113.10:443?encryption=none&security=reality&sni=www.microsoft.com&fp=chrome&pbk=<pubkey>&sid=<shortid>&type=tcp&flow=xtls-rprx-vision#my-node
```

Protocols without a widely-adopted share-link format (AnyTLS, Snell, ShadowTLS,
SOCKS5, HTTP, Mixed) generate a config only; configure those clients manually.

### Examples
```bash
# Run with a single config file
shoes config.yaml

# Run with multiple config files
shoes server1.yaml server2.yaml rules.yaml

# Run with custom thread count
shoes --threads 8 config.yaml

# Validate configuration without starting
shoes --dry-run config.yaml

# Run without hot-reloading
shoes --no-reload config.yaml

# Launch the interactive config wizard
shoes menu

# Generate Reality keypair
shoes generate-reality-keypair

# Generate Shadowsocks 2022 cipher password
shoes generate-shadowsocks-2022-password 2022-blake3-aes-256-gcm

# Generate a VLESS/VMess user ID (UUID v4)
shoes generate-vless-user-id
```

## Configuration

See [CONFIG.md](./CONFIG.md) for the complete YAML configuration reference.

## Examples

See the [examples](./examples) directory for all examples.

### Basic VMess Server
```yaml
- address: 0.0.0.0:16823
  protocol:
    type: vmess
    cipher: chacha20-poly1305
    user_id: b0e80a62-8a51-47f0-91f1-f0f7faf8d9d4
    udp_enabled: true
```

### VLESS with Vision over TLS
```yaml
- address: 0.0.0.0:443
  protocol:
    type: tls
    tls_targets:
      "vless.example.com":
        cert: cert.pem
        key: key.pem
        vision: true
        alpn_protocols: ["http/1.1"]
        protocol:
          type: vless
          user_id: b85798ef-e9dc-46a4-9a87-8da4499d36d0
          udp_enabled: true
```

### Reality Server
```yaml
- address: 0.0.0.0:443
  protocol:
    type: tls
    reality_targets:
      "www.example.com":
        private_key: "YOUR_BASE64URL_PRIVATE_KEY"
        short_ids: ["0123456789abcdef", ""]
        dest: "www.example.com:443"
        protocol:
          type: vless
          user_id: b85798ef-e9dc-46a4-9a87-8da4499d36d0
          udp_enabled: true
```

### Reality Client
```yaml
- address: 127.0.0.1:1080
  protocol:
    type: socks
  rules:
    - masks: "0.0.0.0/0"
      action: allow
      client_chain:
        address: "server.example.com:443"
        protocol:
          type: reality
          public_key: "SERVER_PUBLIC_KEY"
          short_id: "0123456789abcdef"
          sni_hostname: "www.example.com"
          protocol:
            type: vless
            user_id: b85798ef-e9dc-46a4-9a87-8da4499d36d0
```

### Hysteria2 Server
```yaml
- address: 0.0.0.0:443
  transport: quic
  quic_settings:
    cert: cert.pem
    key: key.pem
    alpn_protocols: ["h3"]
  protocol:
    type: hysteria2
    password: supersecret
    udp_enabled: true
```

### TUIC v5 Server
```yaml
- address: 0.0.0.0:443
  transport: quic
  quic_settings:
    cert: cert.pem
    key: key.pem
  protocol:
    type: tuic
    uuid: d685aef3-b3c4-4932-9a9d-d0c2f6727dfa
    password: supersecret
```

### Mixed HTTP/SOCKS5 Server
```yaml
- address: 0.0.0.0:7890
  protocol:
    type: mixed
    username: myuser
    password: mypassword
```

### AnyTLS Server
```yaml
- address: 0.0.0.0:443
  protocol:
    type: tls
    tls_targets:
      "anytls.example.com":
        cert: cert.pem
        key: key.pem
        protocol:
          type: anytls
          users:
            - name: user1
              password: secret123
          udp_enabled: true
```

### NaiveProxy Server
```yaml
- address: 0.0.0.0:443
  protocol:
    type: tls
    tls_targets:
      "naive.example.com":
        cert: cert.pem
        key: key.pem
        alpn_protocols: ["h2"]
        protocol:
          type: naiveproxy
          users:
            - username: user1
              password: secret123
          padding: true
```

### TUN VPN
```yaml
- device_name: tun0
  address: 10.0.0.1
  netmask: 255.255.255.0
  mtu: 1500
  tcp_enabled: true
  udp_enabled: true
  rules:
    - masks: "0.0.0.0/0"
      action: allow
      client_chain:
        address: "proxy.example.com:443"
        protocol:
          type: tls
          protocol:
            type: vless
            user_id: b85798ef-e9dc-46a4-9a87-8da4499d36d0
```

## Similar Projects

- [apernet/hysteria](https://github.com/apernet/hysteria)
- [ihciah/shadow-tls](https://github.com/ihciah/shadow-tls)
- [SagerNet/sing-box](https://github.com/SagerNet/sing-box)
- [shadowsocks/shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust)
- [EAimTY/tuic](https://github.com/EAimTY/tuic)
- [v2fly/v2ray-core](https://github.com/v2fly/v2ray-core)
- [XTLS/Xray-core](https://github.com/XTLS/Xray-core)
