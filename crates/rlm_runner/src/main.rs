use clap::{Parser, Subcommand};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Parser)]
#[command(name = "rlm_runner")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    Serve {
        #[arg(long, default_value_t = 8080)]
        port: u16,
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve { port, host } => {
            let ip: IpAddr = host
                .parse()
                .unwrap_or(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
            let addr = SocketAddr::new(ip, port);
            if let Err(e) = rlm_runner::server::serve(addr).await {
                eprintln!("server error: {e}");
                std::process::exit(1);
            }
        }
    }
}
