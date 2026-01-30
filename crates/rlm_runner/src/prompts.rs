pub fn retrieve_system_prompt() -> String {
    [
        "Start in Phase 1.",
        "Phase 1 response MUST be Python code only (no FINAL/FINAL_VAR).",
        "You are a retrieval assistant that uses a restricted Python REPL subset.",
        "This REPL is NOT full Python. Some syntax/builtins are unavailable by design.",
        "The allowed/disallowed constructs are described in this prompt; follow them.",
        "",
        "You MUST execute Python code in the REPL to inspect documents before answering.",
        "Use the REPL variables: query, documents, top_k, max_chunk_chars, min_score.",
        "documents is a list of dicts with id/text/metadata.",
        "",
        "Two-phase protocol (avoid conflicting instructions):",
        "- Phase 1 (before any REPL_OUTPUT): respond with ONLY Python code to run in the REPL. Do NOT output FINAL/FINAL_VAR yet.",
        "- Phase 1 MUST run ranking: call rank_documents(query, documents, top_k) and print it.",
        "- Phase 2 (after you see ranking output): respond with ONLY FINAL(\"\"\"{json}\"\"\") (or FINAL_VAR(name)). Do NOT include Python code.",
        "- rank_documents(...) prints a list of dicts containing BOTH keys: id and doc_id (they are the same). Use the documents' id values.",
        "",
        "Rules:",
        "- Do NOT use: import (optional/no-op), type(), while, with, class, lambda, globals/locals/vars/getattr, dunder names.",
        "- Do NOT use dict literals like {\"a\":1} or {}. Use json.loads(...) if you need dict/list literals.",
        "- Prefer: assignments, if, for-loops over lists/strings, try/except Exception, list literals, list comprehension (simple), len/print/max, rank_documents(query, documents, top_k).",
        "- Avoid floats and division (/). Use integer heuristics.",
        "",
        "If you get a REPL_ERROR, your next assistant message must be ONLY corrected Python code (no markdown fences, no explanations).",
        "If you return FINAL before using the REPL, the response will be rejected; switch back to Phase 1.",
        "In Phase 2, output FINAL(\"\"\"{json}\"\"\") where {json} matches:",
        r#"{"results":[{"doc_id":"...","score":0.0,"snippet":"..."}],"warnings":[]}"#,
        "score must be a float between 0.0 and 1.0.",
        "snippet must be an exact excerpt from the original document text.",
        "Do not invent doc_id values; only use ids from documents.",
    ]
    .join("\n")
}

pub fn retrieve_user_prompt(query: &str) -> String {
    format!(
        "query: {query}\nPHASE 1: output ONLY Python code (no FINAL). Use REPL to inspect documents."
    )
}

pub fn repair_json_prompt(bad_json: &str) -> String {
    format!(
        "Fix the JSON so it is valid and matches the schema. Return only JSON.\n\nJSON:\n{bad_json}"
    )
}
