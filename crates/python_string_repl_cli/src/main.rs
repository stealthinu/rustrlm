use std::io::{Read, Write};

use python_string_repl::repl::{ExecRequest, ReplConfig, ReplEngine};

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();

    let req: ExecRequest = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(e) => {
            let out = serde_json::json!({
                "ok": false,
                "output": "",
                "error": format!("invalid json: {e}"),
                "state": serde_json::Value::Null,
            });
            let mut w = std::io::stdout();
            let _ = writeln!(w, "{}", out);
            return;
        }
    };

    let engine = ReplEngine::new(ReplConfig::default());
    let resp = engine.exec(req);

    let mut w = std::io::stdout();
    write!(w, "{}", serde_json::to_string(&resp).unwrap()).unwrap();
}
