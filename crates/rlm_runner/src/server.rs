use std::net::SocketAddr;
use axum::extract::State;
use axum::{routing::get, routing::post, Json, Router};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::llm_client::{LlmClient, LlmError, MockLlm, OpenAiClient};
use crate::retrieve::{retrieve, RetrieveContext, RetrieveRequest, RetrieveResponse};

#[derive(Clone)]
pub struct AppState {
    retrieve_ctx: RetrieveContext,
}

impl AppState {
    pub fn new_with_llm(llm: LlmClient) -> Self {
        Self {
            retrieve_ctx: RetrieveContext::new(llm),
        }
    }

    pub fn new_default() -> Result<Self, LlmError> {
        dotenvy::dotenv().ok();
        // Allow running without an LLM (deterministic fallback-only mode).
        if std::env::var("RUSTRLM_DISABLE_LLM")
            .ok()
            .as_deref()
            == Some("1")
        {
            return Ok(Self::new_with_llm(LlmClient::Mock(MockLlm::new(vec![]))));
        }

        let api_key = match std::env::var("OPENAI_API_KEY") {
            Ok(v) => v,
            Err(_) => {
                // No key -> still serve, but rely on fallback retrieval.
                return Ok(Self::new_with_llm(LlmClient::Mock(MockLlm::new(vec![]))));
            }
        };
        let client = OpenAiClient::new(api_key, "gpt-5.2".to_string())?;
        Ok(Self::new_with_llm(LlmClient::OpenAi(client)))
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/version", get(version))
        .route("/v1/retrieve", post(retrieve_handler))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok", "name": "rustrlm", "version": env!("CARGO_PKG_VERSION")}))
}

async fn version() -> Json<serde_json::Value> {
    Json(json!({"name": "rustrlm", "version": env!("CARGO_PKG_VERSION"), "build": "dev"}))
}

async fn retrieve_handler(
    State(state): State<AppState>,
    Json(req): Json<RetrieveRequest>,
) -> Json<RetrieveResponse> {
    Json(retrieve(&req, &state.retrieve_ctx).await)
}

pub async fn serve(addr: SocketAddr) -> std::io::Result<()> {
    let state =
        AppState::new_default().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app(state)).await
}

pub async fn spawn_test_server_with_mock(
    responses: Vec<String>,
) -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = AppState::new_with_llm(LlmClient::Mock(MockLlm::new(responses)));
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app(state)).await;
    });
    (addr, handle)
}

pub async fn spawn_test_server() -> (SocketAddr, JoinHandle<()>) {
    spawn_test_server_with_mock(vec![r#"FINAL("""{"results":[],"warnings":[]}""")"#.to_string()])
        .await
}
