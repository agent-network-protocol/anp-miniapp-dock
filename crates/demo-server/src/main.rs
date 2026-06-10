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
    let state = DemoState::new(config.skill_path);
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
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000),
            skill_path: PathBuf::from("examples/coffee-skill"),
        }
    }
}

impl ServerConfig {
    fn usage() -> &'static str {
        "usage: demo-server [--host 127.0.0.1] [--port 3000] [--skill examples/coffee-skill]"
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
    fn help_exits_without_config() {
        assert_eq!(
            ServerConfig::from_args(["--help".to_owned()]).expect("help parses"),
            None
        );
    }
}
