use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

pub fn retrieve(req: &RetrieveRequest) -> RetrieveResponse {
    let opts = req.options.as_ref();
    let top_k = opts.and_then(|o| o.top_k).unwrap_or(5);
    let max_chunk_chars = opts.and_then(|o| o.max_chunk_chars).unwrap_or(800);
    let min_score = opts.and_then(|o| o.min_score).unwrap_or(0.0);
    let include_spans = opts.and_then(|o| o.include_spans).unwrap_or(true);

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
            score,
            text,
            metadata: doc.metadata.clone(),
            spans,
        });
    }

    RetrieveResponse {
        trace_id: Uuid::new_v4().to_string(),
        results,
        warnings: Vec::new(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_basic() {
        let t = tokenize("Hello, world! 123");
        assert_eq!(t, vec!["hello", "world", "123"]);
    }

    #[test]
    fn centered_slice_bounds() {
        let s = "abcdef";
        let (chunk, start) = centered_slice(s, 3, 3);
        assert_eq!(chunk.len(), 3);
        assert!(start <= 3);
    }
}
