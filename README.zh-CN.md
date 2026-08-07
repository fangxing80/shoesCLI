# shoes

[English](./README.md) | 简体中文

shoes 是一个用 Rust 编写的高性能多协议代理服务器。

## 支持的协议

### 代理协议
- **HTTP/HTTPS**
- **SOCKS5**（支持 UDP ASSOCIATE）
- **Mixed**（自动识别 HTTP/SOCKS5）
- **VMess AEAD**
- **VLESS**（支持 fallback 回落）
- **Shadowsocks**
- **Trojan**
- **Snell v3**
- **Hysteria2**
- **TUIC v5**
- **AnyTLS**
- **NaiveProxy**
- **H2MUX**（支持 VMess、VLESS、Trojan、Shadowsocks、Snell）

### 传输协议
除全部服务端协议外，还支持：
- **SagerNet UDP over TCP**（用于 Shadowsocks、SOCKS5、AnyTLS、NaiveProxy）
- **ShadowTLS v3**
- **TLS**
- **WebSocket**（Shadowsocks SIP003）
- **XTLS Reality**
- **XTLS Vision**（用于 VLESS）

### TUN/VPN 模式
- **TUN 设备支持** —— 三层（Layer 3）VPN，实现透明代理
- 支持平台：Linux、Android、iOS

### 支持的加密算法
- **VMess**：`aes-128-gcm`、`chacha20-poly1305`、`none`
- **Shadowsocks**：`aes-128-gcm`、`aes-256-gcm`、`chacha20-ietf-poly1305`、`2022-blake3-aes-128-gcm`、`2022-blake3-aes-256-gcm`、`2022-blake3-chacha20-ietf-poly1305`
- **Snell v3**：`aes-128-gcm`、`aes-256-gcm`、`chacha20-ietf-poly1305`

## 功能特性

- **多传输层**：所有协议均可选 TCP 或 QUIC
- **TLS SNI 路由**：按服务器名称指示（SNI）分流
- **上游代理链**：多跳链路，支持负载均衡
- **基于规则的路由**：按 IP/CIDR 或域名掩码分流
- **命名 PEM 证书**：定义一次，处处引用
- **TLS 指纹认证**：为 TLS/QUIC 提供证书固定（pinning）
- **热重载**：修改配置无需重启即可生效
- **Unix socket 支持**：可绑定到 Unix 域套接字
- **交互式向导**：通过 `shoes menu` 菜单式生成配置
- **一键安装脚本**：`curl | sh` 安装，Linux 下可选生成 systemd 服务

如需更高级的访问控制（IP 白名单/黑名单），请参见 [tobaru](https://github.com/cfal/tobaru)。

## 安装

### 一键安装（Linux/macOS）

```bash
curl -fsSL https://raw.githubusercontent.com/fangxing80/shoesCLI/master/scripts/install.sh | sh
```

脚本会自动检测操作系统/架构，下载对应的 release 二进制到 `/usr/local/bin`；在带 systemd 的 Linux 上还会安装并启用 `shoes` 服务。

环境变量覆盖：

| 变量 | 默认值 | 用途 |
|---|---|---|
| `SHOES_VERSION` | latest | 要安装的 release 标签 |
| `SHOES_BIN_DIR` | `/usr/local/bin` | 二进制安装目录 |
| `SHOES_CONFIG_DIR` | `/etc/shoes` | 配置目录（systemd） |
| `SHOES_NO_SERVICE` | `0` | 设为 `1` 跳过 systemd 配置 |
| `SHOES_USE_MUSL` | `0` | 设为 `1` 优先使用 Linux 静态 musl 版本 |

### 预编译二进制

x86_64 与 Apple aarch64 的预编译二进制可在 [Github Releases](https://github.com/cfal/shoes/releases) 获取。

### 从源码安装

```bash
cargo install shoes
```

## 使用方法

```
shoes [OPTIONS] <config.yaml> [config.yaml...]

OPTIONS:
    -t, --threads NUM    设置工作线程数（默认：CPU 核数）
    -l, --log-file PATH  输出日志到文件（可重复；"-" 表示 stderr；默认：stderr）
    -d, --dry-run        仅解析配置后退出
    --no-reload          关闭配置文件变更时的自动重载
    -V, --version        打印版本信息后退出

COMMANDS:
    menu                                           启动交互式配置向导（别名：wizard）
    generate-reality-keypair                       生成新的 Reality X25519 密钥对
    generate-shadowsocks-2022-password <cipher>    生成 Shadowsocks 密码
    generate-vless-user-id                         生成随机的 VLESS/VMess 用户 ID（UUID v4）
```

不带配置参数运行时，`shoes` 会从当前目录加载 `config.shoes.yaml`。

### 交互式向导

运行 `shoes menu`（别名 `shoes wizard`）进入菜单式界面，无需手写 YAML 即可生成服务端配置：

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

配置生成器支持 VLESS+REALITY+Vision、VMess、Shadowsocks、Trojan、Hysteria2、TUIC v5、AnyTLS、NaiveProxy、Snell v3、ShadowTLS v3、SOCKS5、HTTP 以及 Mixed（HTTP+SOCKS5）。它会自动生成密钥/UUID/short ID 和 Shadowsocks-2022 密钥，填入合理的默认值，校验结果，并写入 `config.shoes.yaml`（或你指定的路径）。

对于 VLESS+REALITY、VMess、Shadowsocks、Trojan、Hysteria2、TUIC 和 NaiveProxy，向导在保存配置后还会打印客户端**导入链接**（会提示你输入服务器的公网 IP/域名，因为绑定地址通常是 `0.0.0.0`）。将链接粘贴到 v2rayN / Shadowrocket / Clash 系客户端即可导入节点。示例：

```
-- Client import link --
Server public IP or domain (for the share link) [www.microsoft.com]: 203.0.113.10
Node label [shoes]: my-node

  import URL (VLESS-REALITY)
  vless://<uuid>@203.0.113.10:443?encryption=none&security=reality&sni=www.microsoft.com&fp=chrome&pbk=<pubkey>&sid=<shortid>&type=tcp&flow=xtls-rprx-vision#my-node
```

没有通用分享链接格式的协议（AnyTLS、Snell、ShadowTLS、SOCKS5、HTTP、Mixed）只生成配置文件，请手动配置对应客户端。

### 示例
```bash
# 使用单个配置文件运行
shoes config.yaml

# 使用多个配置文件运行
shoes server1.yaml server2.yaml rules.yaml

# 自定义线程数运行
shoes --threads 8 config.yaml

# 仅校验配置而不启动
shoes --dry-run config.yaml

# 关闭热重载运行
shoes --no-reload config.yaml

# 启动交互式配置向导
shoes menu

# 生成 Reality 密钥对
shoes generate-reality-keypair

# 生成 Shadowsocks 2022 加密密码
shoes generate-shadowsocks-2022-password 2022-blake3-aes-256-gcm

# 生成 VLESS/VMess 用户 ID（UUID v4）
shoes generate-vless-user-id
```

## 配置

完整的 YAML 配置参考请见 [CONFIG.md](./CONFIG.md)。

## 示例

所有示例见 [examples](./examples) 目录。

### 基础 VMess 服务器
```yaml
- address: 0.0.0.0:16823
  protocol:
    type: vmess
    cipher: chacha20-poly1305
    user_id: b0e80a62-8a51-47f0-91f1-f0f7faf8d9d4
    udp_enabled: true
```

### 基于 TLS 的 VLESS + Vision
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

### Reality 服务器
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

### Reality 客户端
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

### Hysteria2 服务器
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

### TUIC v5 服务器
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

### Mixed HTTP/SOCKS5 服务器
```yaml
- address: 0.0.0.0:7890
  protocol:
    type: mixed
    username: myuser
    password: mypassword
```

### AnyTLS 服务器
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

### NaiveProxy 服务器
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

## 相似项目

- [apernet/hysteria](https://github.com/apernet/hysteria)
- [ihciah/shadow-tls](https://github.com/ihciah/shadow-tls)
- [SagerNet/sing-box](https://github.com/SagerNet/sing-box)
- [shadowsocks/shadowsocks-rust](https://github.com/shadowsocks/shadowsocks-rust)
- [EAimTY/tuic](https://github.com/EAimTY/tuic)
- [v2fly/v2ray-core](https://github.com/v2fly/v2ray-core)
- [XTLS/Xray-core](https://github.com/XTLS/Xray-core)
