use serde_json::json;

#[tokio::test]
async fn retrieve_falls_back_when_llm_never_returns_final() {
    // The mock LLM returns REPL code but never FINAL(...), forcing the loop to hit max_iterations.
    let responses = vec![
        "print(len(documents))".to_string(),
        "print(len(documents))".to_string(),
        "print(len(documents))".to_string(),
        "print(len(documents))".to_string(),
    ];
    let (addr, _handle) = rlm_runner::server::spawn_test_server_with_mock(responses).await;
    let url = format!("http://{}/v1/retrieve", addr);

    let req = json!({
        "query": "brown fox",
        "documents": [
            {"id": "doc1", "text": "alpha beta gamma"},
            {"id": "doc2", "text": "the quick brown fox jumps"}
        ],
        "options": {"top_k": 1, "max_chunk_chars": 50}
    });

    let client = reqwest::Client::new();
    let resp = client.post(url).json(&req).send().await.unwrap();
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();

    let results = body["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["doc_id"], "doc2");
    assert!(body["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap_or("").contains("fallback_used")));
}
