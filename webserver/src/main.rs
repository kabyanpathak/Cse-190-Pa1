use serde_json::{Value, json};

use axum::{Json, Router, http::StatusCode, response::Html, routing::get};

const BOOGLE: &str = include_str!("../../boogle.html");

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index))
        .route("/jsontest", get(jsontest));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> (StatusCode, Html<&'static str>) {
    (StatusCode::OK, Html(BOOGLE))
}

async fn jsontest() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({"data": "I am in CSE190/CSE291!"})),
    )
}
