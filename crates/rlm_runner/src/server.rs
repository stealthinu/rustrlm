use std::net::SocketAddr;

use axum::{routing::get, routing::post, Json, Router};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::retrieve::{retrieve, RetrieveRequest, RetrieveResponse};

pub fn app() -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/version", get(version))
        .route("/v1/retrieve", post(retrieve_handler))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok", "name": "rustrlm", "version": env!("CARGO_PKG_VERSION")}))
}

async fn version() -> Json<serde_json::Value> {
    Json(json!({"name": "rustrlm", "version": env!("CARGO_PKG_VERSION"), "build": "dev"}))
}

async fn retrieve_handler(Json(req): Json<RetrieveRequest>) -> Json<RetrieveResponse> {
    Json(retrieve(&req))
}

pub async fn serve(addr: SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app()).await
}

pub async fn spawn_test_server() -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app()).await;
    });
    (addr, handle)
}
