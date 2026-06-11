use demo_server::auth::{ServerAuthConfig, TokenIssuerConfig};
use demo_server::{app, DemoState};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

const BINARY_NAME: &str = "demo-server";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(config) = ServerConfig::from_args(std::env::args().skip(1))? else {
        println!("{}", ServerConfig::usage());
        return Ok(());
    };
    let state = DemoState::with_auth_config(config.skill_path, config.auth);
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    println!(
        "{BINARY_NAME} listening on http://{}",
        listener.local_addr()?
    );
    axum::serve(listener, app(state)).await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServerConfig {
    addr: SocketAddr,
    skill_path: PathBuf,
    auth: ServerAuthConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000),
            skill_path: PathBuf::from("examples/coffee-skill"),
            auth: ServerAuthConfig::new("did:wba:coffee-merchant.example"),
        }
    }
}

impl ServerConfig {
    fn usage() -> &'static str {
        "usage: demo-server [--host 127.0.0.1] [--port 3000] [--skill examples/coffee-skill] [--merchant-did did:wba:...] [--token-issuer-secret <secret>] [--trusted-did-document did=path]"
    }

    fn from_args(args: impl IntoIterator<Item = String>) -> Result<Option<Self>, String> {
        let mut config = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--host" => {
                    let host = args.next().ok_or("--host requires a value")?;
                    let ip = host
                        .parse::<IpAddr>()
                        .map_err(|error| format!("invalid --host `{host}`: {error}"))?;
                    config.addr = SocketAddr::new(ip, config.addr.port());
                }
                "--port" => {
                    let port = args.next().ok_or("--port requires a value")?;
                    let port = port
                        .parse::<u16>()
                        .map_err(|error| format!("invalid --port `{port}`: {error}"))?;
                    config.addr.set_port(port);
                }
                "--skill" => {
                    config.skill_path =
                        PathBuf::from(args.next().ok_or("--skill requires a value")?);
                }
                "--merchant-did" => {
                    config.auth.merchant_did =
                        args.next().ok_or("--merchant-did requires a value")?;
                }
                "--token-issuer-secret" => {
                    let secret = args
                        .next()
                        .ok_or("--token-issuer-secret requires a value")?;
                    config.auth.token_issuer = Some(
                        TokenIssuerConfig::new_hs256(secret)
                            .map_err(|_| "invalid --token-issuer-secret".to_owned())?,
                    );
                }
                "--trusted-did-document" => {
                    let value = args
                        .next()
                        .ok_or("--trusted-did-document requires did=path")?;
                    let (did, path) = value
                        .split_once('=')
                        .ok_or("--trusted-did-document requires did=path")?;
                    if did.trim().is_empty() || path.trim().is_empty() {
                        return Err("--trusted-did-document requires did=path".to_owned());
                    }
                    config
                        .auth
                        .trusted_did_documents
                        .insert(did.to_owned(), PathBuf::from(path));
                }
                "--help" | "-h" => {
                    return Ok(None);
                }
                other => return Err(format!("unknown argument `{other}`")),
            }
        }
        Ok(Some(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_binary_name() {
        assert_eq!(BINARY_NAME, "demo-server");
    }

    #[test]
    fn parses_server_args() {
        let config = ServerConfig::from_args([
            "--host".to_owned(),
            "0.0.0.0".to_owned(),
            "--port".to_owned(),
            "3100".to_owned(),
            "--skill".to_owned(),
            "examples/coffee-skill".to_owned(),
        ])
        .expect("args parse");
        let config = config.expect("not help");

        assert_eq!(config.addr.port(), 3100);
        assert_eq!(
            config.skill_path,
            std::path::PathBuf::from("examples/coffee-skill")
        );
    }

    #[test]
    fn parses_auth_config_args_without_printing_secrets() {
        let config = ServerConfig::from_args([
            "--merchant-did".to_owned(),
            "did:wba:merchant.example".to_owned(),
            "--token-issuer-secret".to_owned(),
            "test-only-secret".to_owned(),
            "--trusted-did-document".to_owned(),
            "did:wba:user.example=fixtures/user/did.json".to_owned(),
        ])
        .expect("args parse")
        .expect("not help");

        assert_eq!(config.auth.merchant_did, "did:wba:merchant.example");
        assert_eq!(
            config
                .auth
                .token_issuer
                .as_ref()
                .map(|issuer| issuer.algorithm.as_str()),
            Some("HS256")
        );
        assert_eq!(
            config
                .auth
                .trusted_did_documents
                .get("did:wba:user.example"),
            Some(&PathBuf::from("fixtures/user/did.json"))
        );
        let summary = config
            .auth
            .token_issuer
            .as_ref()
            .expect("issuer")
            .redacted_summary();
        assert_eq!(summary.get("secret"), Some(&"[REDACTED]"));
    }

    #[test]
    fn help_exits_without_config() {
        assert_eq!(
            ServerConfig::from_args(["--help".to_owned()]).expect("help parses"),
            None
        );
    }
}
