use std::time::Duration;

use python_string_repl::repl::{ExecRequest, ReplEngine};
use python_string_repl::repl::state::{ReplState, StoredValue};

use crate::final_parser::{extract_final, extract_final_var_name};
use crate::llm_client::{LlmClient, LlmMessage, LlmRequest};

#[derive(Debug, Clone)]
pub struct RlmLoopConfig {
    pub max_iterations: usize,
    pub max_retries: usize,
    pub request_timeout: Duration,
}

impl Default for RlmLoopConfig {
    fn default() -> Self {
        Self {
            // Match the unofficial baseline harness defaults (paper-ish).
            max_iterations: 20,
            max_retries: 5,
            request_timeout: Duration::from_secs(90),
        }
    }
}

#[derive(Debug)]
pub struct RlmLoopResult {
    pub final_text: Option<String>,
    pub last_response: Option<String>,
    pub last_repl_error: Option<String>,
    pub iterations: usize,
    pub warnings: Vec<String>,
    pub state: ReplState,
}

pub async fn run_rlm_loop(
    llm: &LlmClient,
    repl: &ReplEngine,
    system_prompt: &str,
    user_prompt: &str,
    query: &str,
    mut state: ReplState,
    cfg: &RlmLoopConfig,
) -> RlmLoopResult {
    let mut warnings = Vec::new();
    let mut messages = vec![
        LlmMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        },
        LlmMessage {
            role: "user".to_string(),
            content: user_prompt.to_string(),
        },
    ];
    let mut last_response = None;
    let mut did_repl = false;
    let mut last_repl_error = None;
    let mut iterations = 0usize;
    for _ in 0..cfg.max_iterations {
        iterations += 1;
        let mut attempt = 0usize;
        let content = loop {
            attempt += 1;
            let req = LlmRequest {
                messages: messages.clone(),
                timeout: cfg.request_timeout,
            };
            match llm.complete(req).await {
                Ok(resp) => break resp.content,
                Err(e) if attempt <= cfg.max_retries => {
                    warnings.push(format!("llm_error_retry: {e}"));
                    continue;
                }
                Err(e) => {
                    warnings.push(format!("llm_error: {e}"));
                    return RlmLoopResult {
                        final_text: None,
                        last_response,
                        last_repl_error,
                        iterations,
                        warnings,
                        state,
                    };
                }
            }
        };
        last_response = Some(content.clone());

        // If the model mixes FINAL(...) with code, prefer to run code and ignore FINAL.
        // This is more robust than hard-failing, and helps recover from "eager finalization".
        let (code, _had_code_block) = extract_repl_code(&content);
        let stripped_code = strip_final_lines(&code);
        let has_executable_code = !stripped_code.trim().is_empty();

        if let Some(final_text) = extract_final(&content) {
            if !did_repl {
                warnings.push("final_before_repl".to_string());
                if has_executable_code {
                    warnings.push("final_mixed_with_code_ignored".to_string());
                } else {
                    messages.push(LlmMessage {
                        role: "assistant".to_string(),
                        content,
                    });
                    messages.push(LlmMessage {
                        role: "user".to_string(),
                        content: [
                            "REPL_REQUIRED:",
                            "- You returned FINAL before any REPL execution. That is invalid.",
                            "- Next message MUST be ONLY Python code (no FINAL, no explanations, no markdown fences).",
                            "- Start by ranking and printing: ranked = rank_documents(query, documents, top_k); print(ranked)",
                        ]
                        .join("\n"),
                    });
                    continue;
                }
            } else if has_executable_code {
                warnings.push("final_mixed_with_code_ignored".to_string());
            } else {
                return RlmLoopResult {
                    final_text: Some(final_text),
                    last_response,
                    last_repl_error,
                    iterations,
                    warnings,
                    state,
                };
            }
        }

        if let Some(var_name) = extract_final_var_name(&content) {
            if !did_repl {
                warnings.push("final_var_before_repl".to_string());
                if has_executable_code {
                    warnings.push("final_var_mixed_with_code_ignored".to_string());
                } else {
                    messages.push(LlmMessage {
                        role: "assistant".to_string(),
                        content,
                    });
                    messages.push(LlmMessage {
                        role: "user".to_string(),
                        content: [
                            "REPL_REQUIRED:",
                            "- You returned FINAL_VAR before any REPL execution. That is invalid.",
                            "- Next message MUST be ONLY Python code (no FINAL/FINAL_VAR, no explanations, no markdown fences).",
                            "- Start by ranking and printing: ranked = rank_documents(query, documents, top_k); print(ranked)",
                        ]
                        .join("\n"),
                    });
                    continue;
                }
            } else if has_executable_code {
                warnings.push("final_var_mixed_with_code_ignored".to_string());
            } else {
                match state.get(&var_name) {
                    Some(StoredValue::Str(s)) => {
                        return RlmLoopResult {
                            final_text: Some(s.clone()),
                            last_response,
                            last_repl_error,
                            iterations,
                            warnings,
                            state,
                        };
                    }
                    Some(_) => warnings.push(format!("final_var_not_string: {var_name}")),
                    None => warnings.push(format!("final_var_missing: {var_name}")),
                }
            }
        }

        let exec = repl.exec(ExecRequest {
            context: String::new(),
            query: query.to_string(),
            code: stripped_code,
            max_output_chars: None,
            state: Some(state.clone()),
        });
        let feedback = format_repl_feedback(&exec);
        if !exec.ok {
            if let Some(err) = &exec.error {
                eprintln!("[rlm_loop] repl_error: {err}");
                last_repl_error = Some(err.clone());
            } else {
                eprintln!("[rlm_loop] repl_error: unknown");
                last_repl_error = Some("unknown".to_string());
            }
        }
        did_repl = true;
        state = exec.state.unwrap_or_default();

        messages.push(LlmMessage {
            role: "assistant".to_string(),
            content,
        });
        messages.push(LlmMessage {
            role: "user".to_string(),
            content: feedback,
        });
    }

    warnings.push("final_not_found: max_iterations reached".to_string());
    RlmLoopResult {
        final_text: None,
        last_response,
        last_repl_error,
        iterations,
        warnings,
        state,
    }
}

fn format_repl_feedback(exec: &python_string_repl::repl::ExecResponse) -> String {
    let mut out = String::new();
    if exec.ok {
        out.push_str("REPL_OUTPUT:\n");
        out.push_str(&exec.output);
    } else {
        out.push_str("REPL_ERROR:\n");
        if let Some(err) = &exec.error {
            out.push_str(err);
        }
        out.push_str("\nREPL_OUTPUT:\n");
        out.push_str(&exec.output);
    }
    out
}

fn extract_repl_code(content: &str) -> (String, bool) {
    let mut blocks = Vec::new();
    let mut in_block = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if in_block {
            blocks.push(line);
        }
    }
    if blocks.is_empty() {
        (content.trim().to_string(), false)
    } else {
        (blocks.join("\n").trim().to_string(), true)
    }
}

fn strip_final_lines(code: &str) -> String {
    let mut out = Vec::new();
    for line in code.lines() {
        let t = line.trim();
        if t.starts_with("FINAL(") || t.starts_with("FINAL_VAR(") {
            continue;
        }
        out.push(line);
    }
    out.join("\n").trim().to_string()
}
