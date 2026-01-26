use serde_json::json;

#[tokio::test]
async fn health_endpoint_ok() {
    let (addr, _handle) = rlm_runner::server::spawn_test_server().await;
    let url = format!("http://{}/v1/health", addr);
    let resp = reqwest::get(url).await.unwrap();
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["name"], "rustrlm");
}

#[tokio::test]
async fn retrieve_basic_ranks_relevant_doc() {
    let (addr, _handle) = rlm_runner::server::spawn_test_server().await;
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
    let text = results[0]["text"].as_str().unwrap();
    assert!(text.to_lowercase().contains("brown fox"));
    assert!(results[0]["score"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn retrieve_respects_max_chunk_chars() {
    let (addr, _handle) = rlm_runner::server::spawn_test_server().await;
    let url = format!("http://{}/v1/retrieve", addr);
    let long_text = "x".repeat(2000) + " target " + &"y".repeat(2000);
    let req = json!({
        "query": "target",
        "documents": [
            {"id": "doc1", "text": long_text}
        ],
        "options": {"top_k": 1, "max_chunk_chars": 120}
    });
    let client = reqwest::Client::new();
    let resp = client.post(url).json(&req).send().await.unwrap();
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.unwrap();
    let text = body["results"][0]["text"].as_str().unwrap();
    assert!(text.len() <= 120);
}
