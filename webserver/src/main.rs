use tower_cookies::cookie::SameSite;

use serde_json::{Value, json, to_string};

use axum::{
    Json, Router,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{self, get},
};

use tower_cookies::cookie::time::Duration;
use tower_cookies::{Cookie, CookieManagerLayer, Cookies, cookie};
use uuid::Uuid;

const BOOGLE: &str = include_str!("../../boogle.html");

#[tokio::main]
async fn main() {
    let app = Router::new()
        // starting page
        .route("/", get(index))
        .layer(CookieManagerLayer::new())
        //json test page
        .route("/jsontest", get(jsontest))
        // search functionality
        .route("/search", get(search));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn dump() {}

async fn api() {}

async fn search() -> (StatusCode, impl IntoResponse) {
    (StatusCode::SEE_OTHER, Redirect::to("/"))
}

async fn index(cookies: Cookies) -> (StatusCode, Html<&'static str>) {
    if cookies.get("boogle_id").is_none() {
        let new_id = Uuid::new_v4().to_string();
        cookies.add(
            Cookie::build(("boogle_id", new_id.clone()))
                .path("/")
                .http_only(true)
                .same_site(SameSite::Lax)
                .max_age(Duration::days(365))
                .into(),
        );
    }
    (StatusCode::OK, Html(BOOGLE))
}

async fn jsontest() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({"data": "I am in CSE190/CSE291!"})),
    )
}
