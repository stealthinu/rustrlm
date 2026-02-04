use std::collections::HashMap;
use std::sync::Arc;

use python_string_repl::repl::{ReplConfig, ReplEngine};
use python_string_repl::repl::state::{ReplState, StoredValue};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::llm_client::{LlmClient, LlmError};
use crate::prompts::{repair_json_prompt, retrieve_system_prompt, retrieve_user_prompt};
use crate::rlm_loop::{run_rlm_loop, RlmLoopConfig};

#[derive(Debug, Deserialize)]
pub struct RetrieveRequest {
    pub query: String,
    pub documents: Vec<Document>,
    #[serde(default)]
    pub options: Option<RetrieveOptions>,
}

#[derive(Debug, Deserialize)]
pub struct Document {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct RetrieveOptions {
    pub top_k: Option<usize>,
    pub max_chunk_chars: Option<usize>,
    pub min_score: Option<f64>,
    pub include_spans: Option<bool>,
    // When LLM is enabled, the default is false (so failures are visible).
    // When LLM is disabled, we always use deterministic retrieval.
    #[serde(default)]
    pub use_fallback: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RetrieveResponse {
    pub trace_id: String,
    pub results: Vec<RetrieveResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RetrieveResult {
    pub doc_id: String,
    pub score: f64,
    pub text: String,
    pub metadata: Option<serde_json::Value>,
    pub spans: Vec<Span>,
}

#[derive(Debug, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone)]
pub struct RetrieveContext {
    pub llm: Arc<LlmClient>,
    pub repl: Arc<ReplEngine>,
    pub rlm: RlmLoopConfig,
    pub max_json_repair: usize,
}

impl RetrieveContext {
    pub fn new(llm: LlmClient) -> Self {
        Self {
            llm: Arc::new(llm),
            repl: Arc::new(ReplEngine::new(ReplConfig::default())),
            rlm: RlmLoopConfig::default(),
            max_json_repair: 1,
        }
    }
}

pub async fn retrieve(req: &RetrieveRequest, ctx: &RetrieveContext) -> RetrieveResponse {
    let trace_id = Uuid::new_v4().to_string();
    let opts = req.options.as_ref();
    let top_k = opts.and_then(|o| o.top_k).unwrap_or(5);
    let max_chunk_chars = opts.and_then(|o| o.max_chunk_chars).unwrap_or(800);
    let min_score = opts.and_then(|o| o.min_score).unwrap_or(0.0);
    let include_spans = opts.and_then(|o| o.include_spans).unwrap_or(true);
    let use_fallback_opt = opts.and_then(|o| o.use_fallback);

    let mut warnings = Vec::new();
    if req.query.trim().is_empty() {
        warnings.push("query_empty".to_string());
    }
    if req.documents.is_empty() {
        warnings.push("documents_empty".to_string());
    }

    let state = build_repl_state(req, top_k, max_chunk_chars, min_score);
    let loop_result = run_rlm_loop(
        ctx.llm.as_ref(),
        ctx.repl.as_ref(),
        &retrieve_system_prompt(),
        &retrieve_user_prompt(&req.query),
        &req.query,
        state,
        &ctx.rlm,
    )
    .await;
    warnings.extend(loop_result.warnings);
    warnings.push(format!("debug_rlm_iterations: {}", loop_result.iterations));
    if let Some(err) = loop_result.last_repl_error.as_ref() {
        warnings.push(format!("debug_last_repl_error: {}", truncate_log(err, 200)));
    }

    let llm_enabled = !matches!(ctx.llm.as_ref(), crate::llm_client::LlmClient::Mock(_));
    let use_fallback = if llm_enabled {
        use_fallback_opt.unwrap_or(false)
    } else {
        true
    };

    let Some(final_text) = loop_result.final_text else {
        if let Some(last) = loop_result.last_response.as_ref() {
            eprintln!("[retrieve] final_not_found last_response={}", truncate_log(last, 1200));
        }
        if let Some(err) = loop_result.last_repl_error.as_ref() {
            warnings.push(format!("debug_last_repl_error: {}", truncate_log(err, 300)));
        }
        warnings.push("llm_failed: final_not_found".to_string());
        if use_fallback {
            let (results, extra) =
                fallback_retrieve(req, top_k, max_chunk_chars, min_score, include_spans);
            warnings.push("fallback_used: llm_final_not_found".to_string());
            warnings.extend(extra);
            return RetrieveResponse {
                trace_id,
                results,
                warnings,
            };
        } else {
            return RetrieveResponse {
                trace_id,
                results: Vec::new(),
                warnings,
            };
        }
    };

    let payload = match parse_llm_payload(&final_text) {
        Ok(p) => p,
        Err(e) => {
            warnings.push(format!("llm_json_parse_failed: {e}"));
            eprintln!(
                "[retrieve] parse_failed final_text={}",
                truncate_log(&final_text, 1200)
            );
            let mut repaired = None;
            for _ in 0..ctx.max_json_repair {
                match repair_json_with_llm(ctx, &final_text).await {
                    Ok(Some(fixed)) => {
                        match parse_llm_payload(&fixed) {
                            Ok(p) => {
                                repaired = Some(p);
                                break;
                            }
                            Err(e) => warnings.push(format!("llm_json_repair_failed: {e}")),
                        }
                    }
                    Ok(None) => warnings.push("llm_json_repair_empty".to_string()),
                    Err(e) => warnings.push(format!("llm_json_repair_error: {e}")),
                }
            }
            match repaired {
                Some(p) => p,
                None => {
                    warnings.push("llm_failed: json_parse_failed".to_string());
                    if use_fallback {
                        let (results, extra) =
                            fallback_retrieve(req, top_k, max_chunk_chars, min_score, include_spans);
                        warnings.push("fallback_used: llm_json_parse_failed".to_string());
                        warnings.extend(extra);
                        return RetrieveResponse {
                            trace_id,
                            results,
                            warnings,
                        };
                    } else {
                        return RetrieveResponse {
                            trace_id,
                            results: Vec::new(),
                            warnings,
                        };
                    }
                }
            }
        }
    };

    warnings.extend(payload.warnings.iter().cloned());
    let (results, extra) = build_results(
        &payload.results,
        &req.documents,
        top_k,
        max_chunk_chars,
        min_score,
        include_spans,
    );
    warnings.extend(extra);

    if results.is_empty() {
        if let Some(last) = loop_result.last_response.as_ref() {
            eprintln!("[retrieve] empty_results last_response={}", truncate_log(last, 1200));
        }
        warnings.push("llm_failed: empty_results".to_string());
        if use_fallback {
            let (fb, extra) =
                fallback_retrieve(req, top_k, max_chunk_chars, min_score, include_spans);
            if !fb.is_empty() {
                warnings.push("fallback_used: empty_results".to_string());
                warnings.extend(extra);
                return RetrieveResponse {
                    trace_id,
                    results: fb,
                    warnings,
                };
            }
        }
    }

    RetrieveResponse {
        trace_id,
        results,
        warnings,
    }
}

#[derive(Debug)]
struct LlmPayload {
    results: Vec<LlmResult>,
    warnings: Vec<String>,
}

#[derive(Debug)]
struct LlmResult {
    doc_id: String,
    score: Option<f64>,
    snippet: Option<String>,
}

fn parse_llm_payload(raw: &str) -> Result<LlmPayload, String> {
    let val: JsonValue = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let results_val = val
        .get("results")
        .ok_or("missing results")?
        .as_array()
        .ok_or("results not array")?;
    let mut results = Vec::new();
    let mut warnings = Vec::new();
    for (idx, item) in results_val.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            warnings.push(format!("result_{idx}_not_object"));
            continue;
        };
        let doc_id = obj
            .get("doc_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let score = obj.get("score").and_then(|v| v.as_f64());
        let snippet = obj
            .get("snippet")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let Some(doc_id) = doc_id else {
            warnings.push(format!("result_{idx}_missing_doc_id"));
            continue;
        };
        results.push(LlmResult {
            doc_id,
            score,
            snippet,
        });
    }

    if let Some(ws) = val.get("warnings").and_then(|v| v.as_array()) {
        for w in ws {
            if let Some(s) = w.as_str() {
                warnings.push(s.to_string());
            }
        }
    }

    Ok(LlmPayload { results, warnings })
}

async fn repair_json_with_llm(
    ctx: &RetrieveContext,
    bad_json: &str,
) -> Result<Option<String>, LlmError> {
    let messages = vec![
        crate::llm_client::LlmMessage {
            role: "system".to_string(),
            content: "You fix JSON formatting.".to_string(),
        },
        crate::llm_client::LlmMessage {
            role: "user".to_string(),
            content: repair_json_prompt(bad_json),
        },
    ];
    let resp = ctx
        .llm
        .as_ref()
        .complete(crate::llm_client::LlmRequest {
            messages,
            timeout: ctx.rlm.request_timeout,
        })
        .await?;
    Ok(Some(resp.content))
}

fn build_results(
    items: &[LlmResult],
    docs: &[Document],
    top_k: usize,
    max_chunk_chars: usize,
    min_score: f64,
    include_spans: bool,
) -> (Vec<RetrieveResult>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut results = Vec::new();
    let mut by_id: HashMap<&str, &Document> = HashMap::new();
    for doc in docs {
        by_id.insert(doc.id.as_str(), doc);
    }

    for item in items.iter().take(top_k) {
        let Some(doc) = by_id.get(item.doc_id.as_str()) else {
            warnings.push(format!("doc_id_not_found: {}", item.doc_id));
            continue;
        };
        let raw_score = item.score.unwrap_or(0.0);
        let score = clamp_score(raw_score);
        if raw_score != score {
            warnings.push(format!("score_clamped: {}", item.doc_id));
        }
        if score < min_score {
            continue;
        }

        let (text, spans, span_warn) =
            text_and_spans(doc.text.as_str(), item.snippet.as_deref(), max_chunk_chars, include_spans);
        if let Some(w) = span_warn {
            warnings.push(format!("snippet_not_found: {}", w));
        }

        results.push(RetrieveResult {
            doc_id: doc.id.clone(),
            score,
            text,
            metadata: doc.metadata.clone(),
            spans,
        });
    }

    (results, warnings)
}

fn text_and_spans(
    doc_text: &str,
    snippet: Option<&str>,
    max_chunk_chars: usize,
    include_spans: bool,
) -> (String, Vec<Span>, Option<String>) {
    if let Some(snippet) = snippet {
        if doc_text.contains(snippet) {
            let text = truncate_chars(snippet, max_chunk_chars);
            let spans = if include_spans && !text.is_empty() {
                vec![Span {
                    start: 0,
                    end: text.chars().count(),
                }]
            } else {
                Vec::new()
            };
            return (text, spans, None);
        }
        let fallback = truncate_chars(doc_text, max_chunk_chars);
        return (fallback, Vec::new(), Some(snippet.to_string()));
    }
    let fallback = truncate_chars(doc_text, max_chunk_chars);
    (fallback, Vec::new(), Some("missing_snippet".to_string()))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(1);
    text.chars().take(max_chars).collect()
}

fn clamp_score(score: f64) -> f64 {
    if score.is_nan() {
        0.0
    } else {
        score.clamp(0.0, 1.0)
    }
}

fn truncate_log(text: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(1);
    let mut out = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn fallback_retrieve(
    req: &RetrieveRequest,
    top_k: usize,
    max_chunk_chars: usize,
    min_score: f64,
    include_spans: bool,
) -> (Vec<RetrieveResult>, Vec<String>) {
    let terms = tokenize(&req.query);
    let mut scored: Vec<(usize, f64)> = Vec::new();
    for (i, doc) in req.documents.iter().enumerate() {
        let score = score_doc(&terms, &doc.text);
        if score >= min_score && score > 0.0 {
            scored.push((i, score));
        }
    }
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| req.documents[a.0].id.cmp(&req.documents[b.0].id))
    });

    let mut results = Vec::new();
    for (idx, score) in scored.into_iter().take(top_k) {
        let doc = &req.documents[idx];
        let (text, span) = extract_best_span(&terms, &doc.text, max_chunk_chars);
        let spans = if include_spans {
            span.into_iter()
                .map(|(s, e)| Span { start: s, end: e })
                .collect()
        } else {
            Vec::new()
        };
        results.push(RetrieveResult {
            doc_id: doc.id.clone(),
            score: clamp_score(score),
            text,
            metadata: doc.metadata.clone(),
            spans,
        });
    }

    let mut warnings = Vec::new();
    if results.is_empty() && !req.documents.is_empty() && !terms.is_empty() {
        warnings.push("fallback_no_matches".to_string());
    }
    (results, warnings)
}

fn tokenize(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() >= 2)
        .map(|s| s.to_string())
        .collect()
}

fn score_doc(terms: &[String], text: &str) -> f64 {
    if terms.is_empty() {
        return 0.0;
    }
    let hay = text.to_lowercase();
    let mut score = 0.0;
    for t in terms {
        let mut pos = 0usize;
        while let Some(i) = hay[pos..].find(t) {
            score += 1.0;
            pos += i + t.len();
            if pos >= hay.len() {
                break;
            }
        }
    }
    score
}

fn extract_best_span(
    terms: &[String],
    text: &str,
    max_chars: usize,
) -> (String, Option<(usize, usize)>) {
    if text.is_empty() {
        return (String::new(), None);
    }
    let lower = text.to_lowercase();
    let mut best_pos: Option<(usize, usize)> = None;
    for t in terms {
        if let Some(i) = lower.find(t) {
            let end = i + t.len();
            best_pos = Some((i, end));
            break;
        }
    }
    let (slice, span) = if let Some((start, end)) = best_pos {
        let (chunk, offset) = centered_slice(text, start, max_chars);
        let span_start = start.saturating_sub(offset);
        let span_end = span_start + (end - start);
        (chunk, Some((span_start, span_end)))
    } else {
        let (chunk, _) = centered_slice(text, 0, max_chars);
        (chunk, None)
    };
    (slice, span)
}

fn centered_slice(text: &str, focus: usize, max_chars: usize) -> (String, usize) {
    let total = text.chars().count();
    let max_chars = max_chars.max(1);
    let mut start = if total > max_chars {
        let half = max_chars / 2;
        focus.saturating_sub(half)
    } else {
        0
    };
    if start + max_chars > total {
        start = total.saturating_sub(max_chars);
    }
    let end = (start + max_chars).min(total);
    let slice = text
        .chars()
        .skip(start)
        .take(end - start)
        .collect::<String>();
    (slice, start)
}

fn build_repl_state(
    req: &RetrieveRequest,
    top_k: usize,
    max_chunk_chars: usize,
    min_score: f64,
) -> ReplState {
    let mut state = ReplState::new();
    let mut docs = Vec::new();
    for doc in &req.documents {
        let mut m = HashMap::new();
        m.insert("id".to_string(), StoredValue::Str(doc.id.clone()));
        m.insert("text".to_string(), StoredValue::Str(doc.text.clone()));
        let meta = match &doc.metadata {
            Some(v) => json_to_stored_value(v),
            None => StoredValue::None,
        };
        m.insert("metadata".to_string(), meta);
        docs.push(StoredValue::Dict(m));
    }
    state.insert("documents".to_string(), StoredValue::List(docs));
    state.insert("top_k".to_string(), StoredValue::Int(top_k as i64));
    state.insert(
        "max_chunk_chars".to_string(),
        StoredValue::Int(max_chunk_chars as i64),
    );
    state.insert(
        "min_score".to_string(),
        StoredValue::Str(format!("{min_score:.4}")),
    );
    state
}

fn json_to_stored_value(v: &serde_json::Value) -> StoredValue {
    match v {
        serde_json::Value::Null => StoredValue::None,
        serde_json::Value::Bool(b) => StoredValue::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                StoredValue::Int(i)
            } else {
                StoredValue::Str(n.to_string())
            }
        }
        serde_json::Value::String(s) => StoredValue::Str(s.clone()),
        serde_json::Value::Array(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(json_to_stored_value(x));
            }
            StoredValue::List(out)
        }
        serde_json::Value::Object(m) => {
            let mut out = HashMap::new();
            for (k, v) in m {
                out.insert(k.clone(), json_to_stored_value(v));
            }
            StoredValue::Dict(out)
        }
    }
}
