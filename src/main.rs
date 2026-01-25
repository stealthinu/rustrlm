use std::io::{Read, Write};

use python_string_repl::repl::{ExecRequest, ReplConfig, ReplEngine};

fn main() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        std::process::exit(2);
    }

    let req: ExecRequest = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            let _ = writeln!(
                std::io::stderr(),
                "{{\"ok\":false,\"output\":\"\",\"error\":\"invalid json: {}\"}}",
                e
            );
            std::process::exit(2);
        }
    };

    let engine = ReplEngine::new(ReplConfig::default());
    let resp = engine.exec(req);

    let out = serde_json::to_string(&resp).unwrap_or_else(|_| {
        "{\"ok\":false,\"output\":\"\",\"error\":\"encode error\"}".to_string()
    });
    let _ = std::io::stdout().write_all(out.as_bytes());
}
