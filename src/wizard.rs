//! Interactive command-line wizard for `shoes`.
//!
//! Provides a colorized, menu-driven interface (similar to v2ray-agent) that
//! guides the user through generating a `shoes` server configuration, generating
//! keys/UUIDs, and validating configs.
//!
//! This module is binary-only: it reads from stdin and is not compiled into the
//! library (`lib.rs`) which is used for FFI/mobile embedding.

use std::io::{self, BufRead, Write};

use crate::config;
use crate::reality::generate_keypair;
use crate::shadowsocks::ShadowsocksCipher;
use crate::uuid_util::generate_uuid;

use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use base64::engine::{Engine as _, general_purpose::STANDARD};

const DEFAULT_CONFIG_PATH: &str = "config.shoes.yaml";

// ANSI colors. Disabled automatically when stdout is not a TTY.
struct Palette {
    enabled: bool,
}

impl Palette {
    fn new() -> Self {
        // Enable colors only when stdout is a terminal and NO_COLOR is unset.
        let is_tty = is_stdout_tty();
        let no_color = std::env::var_os("NO_COLOR").is_some();
        Palette {
            enabled: is_tty && !no_color,
        }
    }

    fn wrap(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    fn title(&self, text: &str) -> String {
        self.wrap("1;36", text) // bold cyan
    }

    fn accent(&self, text: &str) -> String {
        self.wrap("1;32", text) // bold green
    }

    fn dim(&self, text: &str) -> String {
        self.wrap("2", text)
    }

    fn warn(&self, text: &str) -> String {
        self.wrap("1;33", text) // bold yellow
    }

    fn err(&self, text: &str) -> String {
        self.wrap("1;31", text) // bold red
    }
}

#[cfg(unix)]
fn is_stdout_tty() -> bool {
    // SAFETY: isatty is a simple libc call with no memory safety concerns.
    unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 }
}

#[cfg(not(unix))]
fn is_stdout_tty() -> bool {
    false
}

/// Entry point for the interactive wizard. Called from `main.rs` for the
/// `menu` / `wizard` subcommands.
pub fn run_wizard() -> io::Result<()> {
    let p = Palette::new();
    let stdin = io::stdin();
    let mut input = stdin.lock();

    loop {
        print_main_menu(&p);
        let choice = match read_choice(&mut input, &p)? {
            Some(c) => c,
            None => {
                // EOF - exit gracefully.
                println!();
                return Ok(());
            }
        };

        match choice.trim() {
            "1" => {
                if let Err(e) = generate_config_flow(&mut input, &p) {
                    println!("{}", p.err(&format!("Error: {e}")));
                }
            }
            "2" => generate_reality_keypair_flow(&p),
            "3" => generate_uuid_flow(&p),
            "4" => generate_ss2022_password_flow(&mut input, &p),
            "5" => {
                if let Err(e) = validate_config_flow(&mut input, &p) {
                    println!("{}", p.err(&format!("Error: {e}")));
                }
            }
            "0" | "q" | "Q" => {
                println!("{}", p.dim("Bye."));
                return Ok(());
            }
            other => {
                println!("{}", p.err(&format!("Invalid option: {other}")));
            }
        }
        println!();
    }
}

fn print_main_menu(p: &Palette) {
    println!();
    println!("{}", p.title("================================"));
    println!("{}", p.title("   shoes  interactive  menu"));
    println!("{}", p.title("================================"));
    println!("  {}  Generate a server config (wizard)", p.accent("1"));
    println!("  {}  Generate a REALITY keypair", p.accent("2"));
    println!("  {}  Generate a VLESS/VMess UUID", p.accent("3"));
    println!("  {}  Generate a Shadowsocks-2022 password", p.accent("4"));
    println!("  {}  Validate a config file (dry-run)", p.accent("5"));
    println!("  {}  Exit", p.accent("0"));
    print!("{}", p.dim("Select an option: "));
    let _ = io::stdout().flush();
}

/// Reads a single line, returning None on EOF.
fn read_line<R: BufRead>(input: &mut R) -> io::Result<Option<String>> {
    let mut line = String::new();
    let n = input.read_line(&mut line)?;
    if n == 0 {
        return Ok(None);
    }
    // Strip trailing newline(s).
    while line.ends_with('\n') || line.ends_with('\r') {
        line.pop();
    }
    Ok(Some(line))
}

fn read_choice<R: BufRead>(input: &mut R, _p: &Palette) -> io::Result<Option<String>> {
    read_line(input)
}

/// Prompt with a label and an optional default. Returns the entered value or the
/// default when the user submits an empty line.
fn prompt<R: BufRead>(
    input: &mut R,
    p: &Palette,
    label: &str,
    default: Option<&str>,
) -> io::Result<String> {
    loop {
        match default {
            Some(d) => print!("{} [{}]: ", label, p.dim(d)),
            None => print!("{label}: "),
        }
        io::stdout().flush()?;

        match read_line(input)? {
            None => {
                // EOF: fall back to default if present, else empty.
                println!();
                return Ok(default.unwrap_or("").to_string());
            }
            Some(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    if let Some(d) = default {
                        return Ok(d.to_string());
                    }
                    println!("{}", p.warn("A value is required."));
                    continue;
                }
                return Ok(trimmed.to_string());
            }
        }
    }
}

/// Prompt for a yes/no answer.
fn prompt_bool<R: BufRead>(
    input: &mut R,
    p: &Palette,
    label: &str,
    default: bool,
) -> io::Result<bool> {
    let hint = if default { "Y/n" } else { "y/N" };
    loop {
        print!("{label} [{}]: ", p.dim(hint));
        io::stdout().flush()?;
        match read_line(input)? {
            None => {
                println!();
                return Ok(default);
            }
            Some(value) => match value.trim().to_lowercase().as_str() {
                "" => return Ok(default),
                "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => println!("{}", p.warn("Please answer y or n.")),
            },
        }
    }
}

/// The set of protocols exposed by the wizard's first version.
#[derive(Clone, Copy)]
enum WizardProtocol {
    VlessReality,
    Vmess,
    Shadowsocks,
    Trojan,
    Hysteria2,
    Socks,
    Http,
}

fn generate_config_flow<R: BufRead>(input: &mut R, p: &Palette) -> io::Result<()> {
    println!();
    println!("{}", p.title("-- Protocol selection --"));
    println!(
        "  {}  VLESS + REALITY + Vision  (recommended)",
        p.accent("1")
    );
    println!("  {}  VMess", p.accent("2"));
    println!("  {}  Shadowsocks", p.accent("3"));
    println!("  {}  Trojan  (over TLS)", p.accent("4"));
    println!("  {}  Hysteria2  (QUIC)", p.accent("5"));
    println!("  {}  SOCKS5", p.accent("6"));
    println!("  {}  HTTP", p.accent("7"));
    print!("{}", p.dim("Select a protocol: "));
    io::stdout().flush()?;

    let selection = match read_line(input)? {
        Some(s) => s,
        None => return Ok(()),
    };

    let protocol = match selection.trim() {
        "1" => WizardProtocol::VlessReality,
        "2" => WizardProtocol::Vmess,
        "3" => WizardProtocol::Shadowsocks,
        "4" => WizardProtocol::Trojan,
        "5" => WizardProtocol::Hysteria2,
        "6" => WizardProtocol::Socks,
        "7" => WizardProtocol::Http,
        other => {
            println!("{}", p.err(&format!("Invalid protocol: {other}")));
            return Ok(());
        }
    };

    let yaml = match protocol {
        WizardProtocol::VlessReality => build_vless_reality(input, p)?,
        WizardProtocol::Vmess => build_vmess(input, p)?,
        WizardProtocol::Shadowsocks => build_shadowsocks(input, p)?,
        WizardProtocol::Trojan => build_trojan(input, p)?,
        WizardProtocol::Hysteria2 => build_hysteria2(input, p)?,
        WizardProtocol::Socks => build_socks_http(input, p, true)?,
        WizardProtocol::Http => build_socks_http(input, p, false)?,
    };

    println!();
    println!("{}", p.title("-- Generated configuration --"));
    println!("{yaml}");

    // Structurally validate the generated YAML before offering to save it.
    // Full semantic validation (create_server_configs) is skipped here because
    // it requires cert files on disk, which the user has not created yet.
    match parse_structural(&yaml) {
        Ok(()) => println!("{}", p.accent("Config is valid.")),
        Err(e) => {
            println!(
                "{}",
                p.err(&format!("Generated config failed validation: {e}"))
            );
            println!(
                "{}",
                p.warn("This is a bug in the wizard - not saving. Please report it.")
            );
            return Ok(());
        }
    }

    let path = prompt(input, p, "Save to file", Some(DEFAULT_CONFIG_PATH))?;
    if std::path::Path::new(&path).exists() {
        let overwrite = prompt_bool(
            input,
            p,
            &format!("File '{path}' exists. Overwrite?"),
            false,
        )?;
        if !overwrite {
            println!("{}", p.dim("Not saved."));
            return Ok(());
        }
    }

    std::fs::write(&path, &yaml)?;
    println!("{}", p.accent(&format!("Saved to {path}")));
    println!(
        "{}",
        p.dim(&format!("Start the server with:  shoes {path}"))
    );
    Ok(())
}

/// Ask for a bind address, defaulting the host to 0.0.0.0 and prompting for a port.
fn prompt_bind_address<R: BufRead>(
    input: &mut R,
    p: &Palette,
    default_port: u16,
) -> io::Result<String> {
    let host = prompt(input, p, "Bind host", Some("0.0.0.0"))?;
    let port = prompt_port(input, p, default_port)?;
    if host.contains(':') && !host.starts_with('[') {
        // IPv6 literal without brackets.
        Ok(format!("[{host}]:{port}"))
    } else {
        Ok(format!("{host}:{port}"))
    }
}

fn prompt_port<R: BufRead>(input: &mut R, p: &Palette, default_port: u16) -> io::Result<u16> {
    let default_str = default_port.to_string();
    loop {
        let value = prompt(input, p, "Port", Some(&default_str))?;
        match value.parse::<u16>() {
            Ok(0) => println!("{}", p.warn("Port must be between 1 and 65535.")),
            Ok(n) => return Ok(n),
            Err(_) => println!("{}", p.warn("Please enter a valid port number.")),
        }
    }
}

/// Prompt for a UUID, offering to auto-generate one on empty input.
fn prompt_uuid<R: BufRead>(input: &mut R, p: &Palette) -> io::Result<String> {
    let generated = generate_uuid();
    let value = prompt(input, p, "User ID (UUID)", Some(&generated))?;
    Ok(value)
}

fn build_vless_reality<R: BufRead>(input: &mut R, p: &Palette) -> io::Result<String> {
    let address = prompt_bind_address(input, p, 443)?;
    let user_id = prompt_uuid(input, p)?;

    // Generate a REALITY keypair for the user.
    let (private_key, public_key) = generate_keypair()?;
    println!();
    println!(
        "{}",
        p.title("-- REALITY keys (save the public key for clients) --")
    );
    println!("  private_key: {private_key}");
    println!("  {}: {public_key}", p.accent("public_key"));
    println!();

    let sni = prompt(
        input,
        p,
        "SNI / camouflage domain (must support TLS 1.3)",
        Some("www.microsoft.com"),
    )?;
    let dest_default = format!("{sni}:443");
    let dest = prompt(input, p, "Fallback dest", Some(&dest_default))?;

    // Generate a random 8-byte (16 hex char) short id.
    let short_id = random_hex(8);
    let vision = prompt_bool(input, p, "Enable Vision (VLESS only)", true)?;

    let yaml = format!(
        "- address: {address}\n\
         \x20 protocol:\n\
         \x20   type: tls\n\
         \x20   reality_targets:\n\
         \x20     \"{sni}\":\n\
         \x20       private_key: \"{private_key}\"\n\
         \x20       short_ids: [\"{short_id}\"]\n\
         \x20       dest: \"{dest}\"\n\
         \x20       vision: {vision}\n\
         \x20       protocol:\n\
         \x20         type: vless\n\
         \x20         user_id: {user_id}\n\
         \x20         udp_enabled: true\n"
    );
    Ok(yaml)
}

fn build_vmess<R: BufRead>(input: &mut R, p: &Palette) -> io::Result<String> {
    let address = prompt_bind_address(input, p, 16823)?;
    let user_id = prompt_uuid(input, p)?;
    let cipher = prompt_cipher(
        input,
        p,
        "VMess cipher",
        &["aes-128-gcm", "chacha20-poly1305", "none"],
        "aes-128-gcm",
    )?;

    let yaml = format!(
        "- address: {address}\n\
         \x20 protocol:\n\
         \x20   type: vmess\n\
         \x20   cipher: {cipher}\n\
         \x20   user_id: {user_id}\n\
         \x20   udp_enabled: true\n"
    );
    Ok(yaml)
}

fn build_shadowsocks<R: BufRead>(input: &mut R, p: &Palette) -> io::Result<String> {
    let address = prompt_bind_address(input, p, 8388)?;
    let cipher = prompt_cipher(
        input,
        p,
        "Shadowsocks cipher",
        &[
            "aes-128-gcm",
            "aes-256-gcm",
            "chacha20-ietf-poly1305",
            "2022-blake3-aes-128-gcm",
            "2022-blake3-aes-256-gcm",
            "2022-blake3-chacha20-ietf-poly1305",
        ],
        "2022-blake3-aes-256-gcm",
    )?;

    // For 2022 ciphers, generate a base64 key of the correct length; otherwise
    // let the user supply a passphrase.
    let password = if cipher.starts_with("2022-blake3-") {
        let generated = generate_ss2022_password(&cipher)
            .unwrap_or_else(|| "REPLACE_WITH_GENERATED_PASSWORD".to_string());
        println!(
            "{}",
            p.dim("A random 2022 key was generated; press enter to accept.")
        );
        prompt(input, p, "Password (base64 key)", Some(&generated))?
    } else {
        prompt(input, p, "Password", None)?
    };

    let yaml = format!(
        "- address: {address}\n\
         \x20 protocol:\n\
         \x20   type: shadowsocks\n\
         \x20   cipher: {cipher}\n\
         \x20   password: \"{password}\"\n\
         \x20   udp_enabled: true\n"
    );
    Ok(yaml)
}

fn build_trojan<R: BufRead>(input: &mut R, p: &Palette) -> io::Result<String> {
    let address = prompt_bind_address(input, p, 443)?;
    let password = prompt(input, p, "Trojan password", None)?;
    let sni = prompt(input, p, "TLS SNI (cert domain)", Some("example.com"))?;
    let cert = prompt(input, p, "Certificate path (PEM)", Some("cert.pem"))?;
    let key = prompt(input, p, "Private key path (PEM)", Some("key.pem"))?;

    let yaml = format!(
        "- address: {address}\n\
         \x20 protocol:\n\
         \x20   type: tls\n\
         \x20   tls_targets:\n\
         \x20     \"{sni}\":\n\
         \x20       cert: {cert}\n\
         \x20       key: {key}\n\
         \x20       protocol:\n\
         \x20         type: trojan\n\
         \x20         password: \"{password}\"\n"
    );
    Ok(yaml)
}

fn build_hysteria2<R: BufRead>(input: &mut R, p: &Palette) -> io::Result<String> {
    let address = prompt_bind_address(input, p, 443)?;
    let password = prompt(input, p, "Hysteria2 password", None)?;
    let cert = prompt(input, p, "Certificate path (PEM)", Some("cert.pem"))?;
    let key = prompt(input, p, "Private key path (PEM)", Some("key.pem"))?;

    let yaml = format!(
        "- address: {address}\n\
         \x20 transport: quic\n\
         \x20 quic_settings:\n\
         \x20   cert: {cert}\n\
         \x20   key: {key}\n\
         \x20   alpn_protocols: [\"h3\"]\n\
         \x20 protocol:\n\
         \x20   type: hysteria2\n\
         \x20   password: \"{password}\"\n\
         \x20   udp_enabled: true\n"
    );
    Ok(yaml)
}

fn build_socks_http<R: BufRead>(input: &mut R, p: &Palette, is_socks: bool) -> io::Result<String> {
    let default_port = if is_socks { 1080 } else { 8080 };
    let address = prompt_bind_address(input, p, default_port)?;
    let want_auth = prompt_bool(input, p, "Require username/password auth", false)?;

    let type_name = if is_socks { "socks" } else { "http" };
    let mut yaml = format!(
        "- address: {address}\n\
         \x20 protocol:\n\
         \x20   type: {type_name}\n"
    );

    if want_auth {
        let username = prompt(input, p, "Username", None)?;
        let password = prompt(input, p, "Password", None)?;
        yaml.push_str(&format!("    username: \"{username}\"\n"));
        yaml.push_str(&format!("    password: \"{password}\"\n"));
    }

    if is_socks {
        yaml.push_str("    udp_enabled: true\n");
    }

    Ok(yaml)
}

/// Prompt for a cipher, presenting a numbered list of choices.
fn prompt_cipher<R: BufRead>(
    input: &mut R,
    p: &Palette,
    label: &str,
    choices: &[&str],
    default: &str,
) -> io::Result<String> {
    println!("{}", p.dim(&format!("{label} options:")));
    for (i, c) in choices.iter().enumerate() {
        let marker = if *c == default { " (default)" } else { "" };
        println!("  {}  {c}{marker}", p.accent(&(i + 1).to_string()));
    }
    loop {
        let value = prompt(input, p, "Choice (number or name)", Some(default))?;
        // Accept either a 1-based index or a literal cipher name.
        if let Ok(idx) = value.parse::<usize>() {
            if idx >= 1 && idx <= choices.len() {
                return Ok(choices[idx - 1].to_string());
            }
            println!("{}", p.warn("Number out of range."));
            continue;
        }
        if choices.contains(&value.as_str()) {
            return Ok(value);
        }
        println!(
            "{}",
            p.warn("Unknown cipher; pick a number or a listed name.")
        );
    }
}

fn generate_reality_keypair_flow(p: &Palette) {
    match generate_keypair() {
        Ok((private_key, public_key)) => {
            println!();
            println!("{}", p.title("-- REALITY keypair --"));
            println!("  private_key (server): {private_key}");
            println!("  {} (client):  {public_key}", p.accent("public_key"));
        }
        Err(e) => println!("{}", p.err(&format!("Failed to generate keypair: {e}"))),
    }
}

fn generate_uuid_flow(p: &Palette) {
    let uuid = generate_uuid();
    println!();
    println!("{}", p.title("-- UUID (v4) --"));
    println!("  {}", p.accent(&uuid));
}

fn generate_ss2022_password_flow<R: BufRead>(input: &mut R, p: &Palette) {
    let cipher = match prompt(
        input,
        p,
        "Shadowsocks-2022 cipher",
        Some("2022-blake3-aes-256-gcm"),
    ) {
        Ok(c) => c,
        Err(_) => return,
    };
    match generate_ss2022_password(&cipher) {
        Some(password) => {
            println!();
            println!("{}", p.title("-- Shadowsocks-2022 password --"));
            println!("  cipher:   {cipher}");
            println!("  {}: {password}", p.accent("password"));
        }
        None => println!(
            "{}",
            p.err(&format!(
                "'{cipher}' is not a valid 2022-blake3-* cipher; password generation is only needed for those."
            ))
        ),
    }
}

fn validate_config_flow<R: BufRead>(input: &mut R, p: &Palette) -> io::Result<()> {
    let path = prompt(
        input,
        p,
        "Config file to validate",
        Some(DEFAULT_CONFIG_PATH),
    )?;
    match validate_config_file(&path) {
        Ok(()) => {
            println!("{}", p.accent(&format!("{path} is valid.")));
            Ok(())
        }
        Err(e) => {
            println!("{}", p.err(&format!("{path} is invalid: {e}")));
            Ok(())
        }
    }
}

/// Structurally parse a config YAML string, catching YAML syntax, unknown-field,
/// and type errors. Does not load cert files or resolve group references.
fn parse_structural(yaml: &str) -> Result<(), String> {
    config::parse_configs_str(yaml)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Fully validate a config file on disk using the same pipeline as `--dry-run`:
/// load, convert cert paths from files, then build server configs. Runs the
/// async steps on a small single-threaded runtime.
fn validate_config_file(path: &str) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|e| e.to_string())?;

    runtime.block_on(async {
        let args = vec![path.to_string()];
        let configs = config::load_configs(&args)
            .await
            .map_err(|e| e.to_string())?;
        let (configs, _) = config::convert_cert_paths(configs)
            .await
            .map_err(|e| e.to_string())?;
        config::create_server_configs(configs)
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}

/// Generate a base64-encoded Shadowsocks-2022 key of the correct length for the
/// given cipher. Returns None if the cipher is not a valid 2022-blake3-* cipher.
fn generate_ss2022_password(cipher: &str) -> Option<String> {
    let base = cipher.strip_prefix("2022-blake3-")?;
    let cipher = ShadowsocksCipher::try_from(base).ok()?;
    let rng = SystemRandom::new();
    let mut key_bytes = vec![0u8; cipher.key_len()];
    rng.fill(&mut key_bytes).ok()?;
    Some(STANDARD.encode(&key_bytes))
}

/// Generate `n` random bytes rendered as a lowercase hex string (2*n chars).
fn random_hex(n: usize) -> String {
    use std::fmt::Write as _;
    let rng = SystemRandom::new();
    let mut bytes = vec![0u8; n];
    // On the rare chance the RNG fails, fall back to a fixed pattern rather than
    // panicking inside the wizard.
    if rng.fill(&mut bytes).is_err() {
        return "0123456789abcdef".to_string();
    }
    let mut s = String::with_capacity(n * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}
