use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use lettre::{
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        client::{Tls, TlsParametersBuilder},
    },
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};

use super::types::{SmtpConfig, SmtpSendResult};

/// Decode a base64url-encoded string (Gmail format) to raw bytes.
fn decode_base64url(input: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

/// Resolve a hostname to an IPv4 address string, if one exists.
///
/// Cloud containers (e.g. Railway) often have no outbound IPv6 route. Left to
/// its own DNS, lettre may pick the host's AAAA (IPv6) record and fail with
/// "Network is unreachable (os error 101)". We resolve to IPv4 ourselves and
/// connect by IP, while keeping the original hostname for TLS SNI / cert checks.
/// Returns `None` if no IPv4 address is found (e.g. an IPv6-only host).
async fn resolve_ipv4(host: &str, port: u16) -> Option<String> {
    match tokio::net::lookup_host((host, port)).await {
        Ok(addrs) => addrs
            .into_iter()
            .find(|a| a.is_ipv4())
            .map(|a| a.ip().to_string()),
        Err(_) => None,
    }
}

/// Build the TLS parameters for a connection. The hostname (not the IP we may
/// connect to) is used so SNI and certificate validation are correct.
fn tls_params(config: &SmtpConfig) -> Result<lettre::transport::smtp::client::TlsParameters, String> {
    let mut builder = TlsParametersBuilder::new(config.host.clone());
    if config.accept_invalid_certs {
        builder = builder
            .dangerous_accept_invalid_certs(true)
            .dangerous_accept_invalid_hostnames(true);
    }
    builder
        .build()
        .map_err(|e| format!("SMTP TLS params error: {}", e))
}

/// Build an async SMTP transport from the given config.
async fn build_transport(
    config: &SmtpConfig,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, String> {
    let credentials = Credentials::new(config.username.clone(), config.password.clone());

    // For OAuth2, force XOAUTH2 mechanism; for password, use default mechanisms
    let auth_mechanisms = if config.auth_method == "oauth2" {
        vec![Mechanism::Xoauth2]
    } else {
        vec![Mechanism::Plain, Mechanism::Login]
    };

    // Connect by IPv4 address when resolvable; fall back to the hostname.
    let connect_host = resolve_ipv4(&config.host, config.port)
        .await
        .unwrap_or_else(|| config.host.clone());

    let transport = match config.security.as_str() {
        "tls" => {
            // Implicit TLS (typically port 465). TLS params use the real
            // hostname even though we connect by IP.
            AsyncSmtpTransport::<Tokio1Executor>::relay(&connect_host)
                .map_err(|e| format!("SMTP relay error: {}", e))?
                .port(config.port)
                .credentials(credentials)
                .authentication(auth_mechanisms)
                .tls(Tls::Wrapper(tls_params(config)?))
                .build()
        }
        "starttls" => {
            // STARTTLS (typically port 587).
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&connect_host)
                .map_err(|e| format!("SMTP STARTTLS error: {}", e))?
                .port(config.port)
                .credentials(credentials)
                .authentication(auth_mechanisms)
                .tls(Tls::Required(tls_params(config)?))
                .build()
        }
        _ => {
            // Plain / no encryption (typically port 25) — not recommended
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&connect_host)
                .port(config.port)
                .credentials(credentials)
                .authentication(auth_mechanisms)
                .build()
        }
    };

    Ok(transport)
}

/// Extract an SMTP envelope (sender + recipients) from raw RFC 2822 bytes.
///
/// The envelope tells the SMTP server who the mail is from and who to deliver
/// it to, which is separate from the header fields visible to the recipient.
fn extract_envelope(raw: &[u8]) -> Result<lettre::address::Envelope, String> {
    let message = mail_parser::MessageParser::default()
        .parse(raw)
        .ok_or("Failed to parse email for envelope extraction")?;

    // Extract From address
    let from = message
        .from()
        .and_then(|list| list.first())
        .and_then(|addr| addr.address())
        .ok_or("No From address found in email")?;

    let from_addr: lettre::Address = from
        .parse()
        .map_err(|e| format!("Invalid From address '{}': {}", from, e))?;

    // Collect all recipient addresses (To, Cc, Bcc)
    let mut recipients: Vec<lettre::Address> = Vec::new();

    if let Some(to_list) = message.to() {
        for addr in to_list.iter() {
            if let Some(email) = addr.address() {
                if let Ok(a) = email.parse::<lettre::Address>() {
                    recipients.push(a);
                }
            }
        }
    }

    if let Some(cc_list) = message.cc() {
        for addr in cc_list.iter() {
            if let Some(email) = addr.address() {
                if let Ok(a) = email.parse::<lettre::Address>() {
                    recipients.push(a);
                }
            }
        }
    }

    if let Some(bcc_list) = message.bcc() {
        for addr in bcc_list.iter() {
            if let Some(email) = addr.address() {
                if let Ok(a) = email.parse::<lettre::Address>() {
                    recipients.push(a);
                }
            }
        }
    }

    if recipients.is_empty() {
        return Err("No recipients found in email".to_string());
    }

    lettre::address::Envelope::new(Some(from_addr), recipients)
        .map_err(|e| format!("Envelope error: {}", e))
}

/// Send a pre-built RFC 2822 email via SMTP.
///
/// The `raw_email_base64url` parameter is the full email message encoded as
/// base64url (the same encoding Gmail uses: `+` → `-`, `/` → `_`, no padding).
/// The function decodes it, extracts the envelope from headers, and sends it.
pub async fn send_raw_email(
    config: &SmtpConfig,
    raw_email_base64url: &str,
) -> Result<SmtpSendResult, String> {
    let raw_bytes = decode_base64url(raw_email_base64url)?;
    let envelope = extract_envelope(&raw_bytes)?;
    let transport = build_transport(config).await?;

    transport
        .send_raw(&envelope, &raw_bytes)
        .await
        .map(|_response| SmtpSendResult {
            success: true,
            message: "Email sent successfully".to_string(),
        })
        .map_err(|e| format!("SMTP send error: {}", e))
}

/// Test SMTP connectivity by connecting, authenticating, and disconnecting.
pub async fn test_connection(config: &SmtpConfig) -> Result<SmtpSendResult, String> {
    let transport = build_transport(config).await?;

    transport
        .test_connection()
        .await
        .map(|success| SmtpSendResult {
            success,
            message: if success {
                "Connection successful".to_string()
            } else {
                "Connection failed".to_string()
            },
        })
        .map_err(|e| format!("SMTP test error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_ipv4_returns_ipv4_for_localhost() {
        // localhost resolves to 127.0.0.1 (IPv4) on essentially every system.
        let ip = resolve_ipv4("localhost", 587).await;
        if let Some(ip) = ip {
            assert!(ip.parse::<std::net::Ipv4Addr>().is_ok(), "expected IPv4, got {ip}");
        }
        // If localhost is IPv6-only in some sandbox, resolve_ipv4 returns None,
        // and build_transport falls back to the hostname — also acceptable.
    }

    #[tokio::test]
    async fn test_resolve_ipv4_none_for_bogus_host() {
        let ip = resolve_ipv4("nonexistent.invalid.bogus.host.example", 587).await;
        assert!(ip.is_none());
    }

    #[test]
    fn test_decode_base64url_valid() {
        // "Hello" in base64url
        let encoded = "SGVsbG8";
        let decoded = decode_base64url(encoded).unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn test_decode_base64url_invalid() {
        let result = decode_base64url("!!!invalid!!!");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Base64 decode error"));
    }

    #[test]
    fn test_extract_envelope_valid() {
        let raw = b"From: alice@example.com\r\nTo: bob@example.com\r\nCc: carol@example.com\r\nSubject: Test\r\n\r\nBody";
        let envelope = extract_envelope(raw).unwrap();
        // Envelope should have from and 2 recipients (To + Cc)
        assert!(envelope.from().is_some());
        assert_eq!(envelope.to().len(), 2);
    }

    #[test]
    fn test_extract_envelope_no_from() {
        let raw = b"To: bob@example.com\r\nSubject: Test\r\n\r\nBody";
        let result = extract_envelope(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No From address"));
    }

    #[test]
    fn test_extract_envelope_no_recipients() {
        let raw = b"From: alice@example.com\r\nSubject: Test\r\n\r\nBody";
        let result = extract_envelope(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No recipients found"));
    }

    #[test]
    fn test_extract_envelope_with_bcc() {
        let raw = b"From: alice@example.com\r\nTo: bob@example.com\r\nBcc: secret@example.com\r\nSubject: Test\r\n\r\nBody";
        let envelope = extract_envelope(raw).unwrap();
        assert_eq!(envelope.to().len(), 2);
    }
}
