use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "rlm_runner")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    Run {
        #[arg(long)]
        out_jsonl: String,
        #[arg(long)]
        transcript_jsonl: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run {
            out_jsonl: _,
            transcript_jsonl: _,
        } => {
            eprintln!("TODO: implement rlm_runner");
            std::process::exit(2);
        }
    }
}
