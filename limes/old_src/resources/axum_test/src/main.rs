use axum::{Router, body::Body, response::Json, routing::get};
use serde_json::{Value, json};
use tokio;

async fn plain_text() -> &'static str {
    "Porco dio"
}

async fn json() -> Json<Value> {
    Json(json!({ "data": 42 }))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/plain_text", get(plain_text))
        .route("/json", get(json));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
