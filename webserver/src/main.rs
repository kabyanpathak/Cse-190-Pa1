use axum::{
    Json, Router,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::get,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use std::net::SocketAddr;
use tower_cookies::{
    Cookie, CookieManagerLayer, Cookies,
    cookie::{SameSite, time::Duration},
};
use uuid::Uuid;

const BOOGLE: &str = include_str!("../../boogle.html");

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
}

#[derive(Serialize)]
struct SearchItem {
    query: String,
    ts: String,
}
#[derive(Serialize)]
struct HistoryResponse {
    client_id: String,
    searches: Vec<SearchItem>,
}
#[derive(Serialize)]
struct ClientDump {
    client_id: String,
    first_seen: String,
    last_seen: String,
    latest_ip: String,
    ips: Vec<String>,
    searches: Vec<SearchItem>,
}
#[derive(Serialize)]
struct DumpResponse {
    clients: Vec<ClientDump>,
}

#[tokio::main]
async fn main() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(10)
        .connect("sqlite:boogle.db?mode=rwc")
        .await
        .expect("Could not connect to database");

    let app = Router::new()
        .route("/", get(index))
        .route("/jsontest", get(jsontest))
        .route("/search", get(search))
        .route("/api/history", get(api))
        .route("/dump", get(dump))
        .layer(CookieManagerLayer::new())
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5000").await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn dump(State(pool): State<SqlitePool>) -> Json<DumpResponse> {
    let client_rows =
        sqlx::query("SELECT uuid, first_seen, last_seen, latest_ip FROM CLIENT_TABLE")
            .fetch_all(&pool)
            .await
            .unwrap_or_default();
    let mut clients = Vec::new();
    for crow in client_rows {
        let uuid: String = crow.get("uuid");
        let ip_rows = sqlx::query("SELECT DISTINCT ip FROM IPS WHERE uuid = ?")
            .bind(&uuid)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();
        let mut ips = Vec::new();
        for ip_row in ip_rows {
            ips.push(ip_row.get("ip"));
        }
        let search_rows =
            sqlx::query("SELECT query, ts FROM SEARCH_TABLE WHERE uuid = ? ORDER BY id DESC")
                .bind(&uuid)
                .fetch_all(&pool)
                .await
                .unwrap_or_default();
        let mut searches = Vec::new();
        for s_row in search_rows {
            searches.push(SearchItem {
                query: s_row.get("query"),
                ts: s_row.get("ts"),
            });
        }
        clients.push(ClientDump {
            client_id: uuid,
            first_seen: crow.get("first_seen"),
            last_seen: crow.get("last_seen"),
            latest_ip: crow.get("latest_ip"),
            ips,
            searches,
        });
    }
    Json(DumpResponse { clients })
}

async fn api(cookies: Cookies, State(pool): State<SqlitePool>) -> Json<HistoryResponse> {
    let client_id = match cookies.get("boogle_id") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return Json(HistoryResponse {
                client_id: String::new(),
                searches: vec![],
            });
        }
    };
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let _ = sqlx::query("UPDATE CLIENT_TABLE SET last_seen = ? WHERE uuid = ?")
        .bind(&now)
        .bind(&client_id)
        .execute(&pool)
        .await;
    let rows = sqlx::query("SELECT query, ts FROM SEARCH_TABLE WHERE uuid = ? ORDER BY id DESC")
        .bind(&client_id)
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
    let mut searches = Vec::new();
    for row in rows {
        searches.push(SearchItem {
            query: row.get("query"),
            ts: row.get("ts"),
        });
    }
    Json(HistoryResponse {
        client_id,
        searches,
    })
}

async fn search(
    Query(params): Query<SearchParams>,
    cookies: Cookies,
    State(pool): State<SqlitePool>,
) -> (StatusCode, impl IntoResponse) {
    let query_text = params.q.unwrap_or_default();

    if query_text.trim().is_empty() {
        return (StatusCode::SEE_OTHER, Redirect::to("/"));
    }

    let client_id = match cookies.get("boogle_id") {
        Some(cookie) => cookie.value().to_string(),
        None => "".to_string(),
    };

    let ts = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    let _ = sqlx::query(
        "
        INSERT INTO SEARCH_TABLE (query, ts, uuid) VALUES (?, ?, ?)
        ",
    )
    .bind(&query_text)
    .bind(&ts)
    .bind(&client_id)
    .execute(&pool)
    .await;

    (StatusCode::SEE_OTHER, Redirect::to("/"))
}

async fn index(
    cookies: Cookies,
    headers: HeaderMap,
    State(pool): State<SqlitePool>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> (StatusCode, Html<&'static str>) {
    let user_ip = headers
        .get("x-forwarded-for")
        .and_then(|val| val.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());
    let client_id = match cookies.get("boogle_id") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            let new_id = Uuid::new_v4().to_string();
            let cookie = Cookie::build(("boogle_id", new_id.clone()))
                .path("/")
                .http_only(true)
                .same_site(SameSite::Lax)
                .max_age(Duration::days(35))
                .build();
            cookies.add(cookie);
            new_id
        }
    };
    let ts = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    let existing_client = sqlx::query("SELECT latest_ip FROM CLIENT_TABLE WHERE uuid = ?")
        .bind(&client_id)
        .fetch_optional(&pool)
        .await;

    match existing_client {
        Ok(None) => {
            let _ = sqlx::query(
                "INSERT INTO CLIENT_TABLE (uuid, first_seen, last_seen, latest_ip) VALUES (?, ?, ?, ?)"
            )
            .bind(&client_id)
            .bind(&ts)
            .bind(&ts)
            .bind(&user_ip)
            .execute(&pool)
            .await;
            let _ = sqlx::query("INSERT INTO IPS (ip, uuid) VALUES (?, ?)")
                .bind(&user_ip)
                .bind(&client_id)
                .execute(&pool)
                .await;
        }
        Ok(Some(row)) => {
            let _ = sqlx::query("UPDATE CLIENT_TABLE SET last_seen = ? WHERE uuid = ?")
                .bind(&ts)
                .bind(&client_id)
                .execute(&pool)
                .await;
            let old_latest_ip: String = row.get("latest_ip");
            if user_ip != old_latest_ip {
                let _ = sqlx::query("UPDATE CLIENT_TABLE SET latest_ip = ? WHERE uuid = ?")
                    .bind(&user_ip)
                    .bind(&client_id)
                    .execute(&pool)
                    .await;
                let ip_exists = sqlx::query("SELECT id FROM IPS WHERE uuid = ? AND ip = ?")
                    .bind(&client_id)
                    .bind(&user_ip)
                    .fetch_optional(&pool)
                    .await;
                if let Ok(None) = ip_exists {
                    let _ = sqlx::query("INSERT INTO IPS (ip, uuid) VALUES (?, ?)")
                        .bind(&user_ip)
                        .bind(&client_id)
                        .execute(&pool)
                        .await;
                }
            }
        }
        Err(e) => eprintln!("DB Error: {}", e),
    }

    (StatusCode::OK, Html(BOOGLE))
}

async fn jsontest() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({"data": "I am in CSE190/CSE291!"})),
    )
}
