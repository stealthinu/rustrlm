mod allowlist;
mod builtins;
mod eval;
mod parse;
pub mod state;
mod value;

pub use value::Value;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ReplConfig {
    pub max_output_chars: usize,
    pub max_zlib_output_bytes: usize,
    pub max_print_state_chars: usize,
}

impl Default for ReplConfig {
    fn default() -> Self {
        Self {
            max_output_chars: 2000,
            max_zlib_output_bytes: 1_000_000,
            max_print_state_chars: 100_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecRequest {
    pub context: String,
    pub query: String,
    pub code: String,
    #[serde(default)]
    pub max_output_chars: Option<usize>,
    #[serde(default)]
    pub state: Option<state::ReplState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResponse {
    pub ok: bool,
    pub output: String,
    pub error: Option<String>,
    #[serde(default)]
    pub state: Option<state::ReplState>,
}

pub struct ReplEngine {
    cfg: ReplConfig,
}

fn format_error(e: &crate::error::ReplError) -> String {
    use crate::error::ReplError;
    let needs_hint = matches!(
        e,
        ReplError::ForbiddenSyntax(_)
            | ReplError::ForbiddenName(_)
            | ReplError::NameError(_)
            | ReplError::ParseError(_)
    );
    if needs_hint {
        format!("{}\n\n{}", e, ReplError::subset_hint())
    } else {
        e.to_string()
    }
}

impl ReplEngine {
    pub fn new(cfg: ReplConfig) -> Self {
        Self { cfg }
    }

    pub fn exec(&self, req: ExecRequest) -> ExecResponse {
        if req.code.trim().is_empty() {
            return ExecResponse {
                ok: true,
                output: "No code to execute".to_string(),
                error: None,
                state: Some(req.state.unwrap_or_default()),
            };
        }

        let cfg = ReplConfig {
            max_output_chars: req.max_output_chars.unwrap_or(self.cfg.max_output_chars),
            max_zlib_output_bytes: self.cfg.max_zlib_output_bytes,
            max_print_state_chars: self.cfg.max_print_state_chars,
        };

        let base_state = req.state.clone().unwrap_or_default();

        let program = match parse::parse_program(&req.code) {
            Ok(p) => p,
            Err(e) => {
                return ExecResponse {
                    ok: false,
                    output: String::new(),
                    error: Some(format_error(&e)),
                    state: Some(base_state),
                };
            }
        };

        let mut sink = builtins::PrintSink::new(cfg.max_output_chars, cfg.max_print_state_chars);
        let mut env =
            builtins::make_initial_env(cfg.max_zlib_output_bytes, &req.context, &req.query);
        if let Some(st) = req.state {
            if let Err(e) = env.apply_state(&st) {
                return ExecResponse {
                    ok: false,
                    output: String::new(),
                    error: Some(format_error(&e)),
                    state: Some(base_state),
                };
            }
        }

        // RestrictedPython creates a new `_print` collector when code uses `print(...)`,
        // overwriting any stale collector even if compilation/execution later errors.
        //
        // We emulate that with a conservative text check so it still applies even when
        // our allowlist rejects the code (the upstream would have attempted it anyway).
        if req.code.contains("print(") || req.code.contains("print (") {
            env.set("_print_txt", Value::Str(String::new()));
        }

        if let Err(e) = allowlist::validate(&program) {
            return ExecResponse {
                ok: false,
                output: String::new(),
                error: Some(format_error(&e)),
                state: Some(env.dump_state()),
            };
        }

        match eval::exec_program(&program, &mut env, &mut sink) {
            Ok(()) => {
                // If this execution didn't print anything, the upstream executor can leak the
                // previous `_print` collector contents into output.
                if !sink.had_print_call() {
                    if let Some(Value::Str(s)) = env.get("_print_txt") {
                        if sink.push_raw_output(&s).is_err() {
                            // ignore truncation/limits: sink already holds best-effort output
                        }
                    }
                }

                // Echo the last expression (upstream behavior) after collecting print output.
                eval::maybe_echo_last_expr(&req.code, &program, &mut env, &mut sink);

                // Persist the latest print output for the next call.
                if let Some(s) = sink.print_state_snapshot() {
                    env.set("_print_txt", Value::Str(s.to_string()));
                }

                let output = sink.finish();
                let state = env.dump_state();
                ExecResponse {
                    ok: true,
                    output,
                    error: None,
                    state: Some(state),
                }
            }
            Err(e) => {
                // Persist whatever print output happened before the error.
                if let Some(s) = sink.print_state_snapshot() {
                    env.set("_print_txt", Value::Str(s.to_string()));
                }
                let state = env.dump_state();
                ExecResponse {
                    ok: false,
                    output: String::new(),
                    error: Some(format_error(&e)),
                    state: Some(state),
                }
            }
        }
    }
}
