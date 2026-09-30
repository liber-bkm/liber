use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Form;

use liber_core::auth::{
    auth_mac, check_bearer, check_cookie, check_token, same_origin, COOKIE_NAME,
};
use serde::Deserialize;

use crate::AppState;

fn cookie_value(cookie_header: Option<&str>) -> Option<String> {
    let header = cookie_header?;
    for part in header.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix(&format!("{COOKIE_NAME}=")) {
            return Some(v.to_string());
        }
    }
    None
}

fn unauthorized_json() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "application/json")],
        "{\"error\":\"authentication required\"}\n",
    )
        .into_response()
}

fn redirect_login(next: &str) -> Response {
    (
        StatusCode::SEE_OTHER,
        [(
            header::LOCATION,
            format!("/login?next={}", urlencoding(next)),
        )],
        "",
    )
        .into_response()
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub async fn auth_middleware(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if state.token.is_empty() {
        return next.run(req).await;
    }
    let path = req.uri().path().to_string();
    if path == "/login" || path == "/logout" {
        return next.run(req).await;
    }
    let headers = req.headers().clone();
    if let Some(auth) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if check_bearer(auth, &state.token) {
            return next.run(req).await;
        }
    }
    if let Some(cookie) = cookie_value(headers.get(header::COOKIE).and_then(|v| v.to_str().ok())) {
        if check_cookie(&cookie, &state.token) {
            if matches!(req.method().as_str(), "POST" | "PUT" | "DELETE" | "PATCH") {
                let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
                let referer = headers.get(header::REFERER).and_then(|v| v.to_str().ok());
                let host = headers
                    .get(header::HOST)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if !same_origin(origin, referer, host) {
                    return (StatusCode::FORBIDDEN, "cross-origin request refused").into_response();
                }
            }
            return next.run(req).await;
        }
    }
    if path.starts_with("/api/") {
        return unauthorized_json();
    }
    redirect_login(req.uri().path())
}

const LOGIN_FORM: &str = "<!doctype html><html><body><h2>liber login</h2><form method=\"post\"><input type=\"password\" name=\"token\" required><button type=\"submit\">Log in</button></form></body></html>";

pub async fn login_page() -> (
    StatusCode,
    [(header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        LOGIN_FORM,
    )
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub token: String,
    pub next: Option<String>,
}

pub async fn login_submit(State(state): State<AppState>, Form(form): Form<LoginForm>) -> Response {
    if state.token.is_empty() {
        return redirect_login("/");
    }
    if !check_token(&form.token, &state.token) {
        return (StatusCode::UNAUTHORIZED, "wrong token").into_response();
    }
    let dest = form.next.unwrap_or_else(|| "/".to_string());
    let dest = if dest.starts_with('/') {
        dest
    } else {
        "/".to_string()
    };
    let cookie = format!(
        "{COOKIE_NAME}={}; Path=/; HttpOnly; SameSite=Lax",
        auth_mac(&state.token, "liber-cookie-v1")
    );
    (
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, cookie.as_str()),
            (header::LOCATION, dest.as_str()),
        ],
        "",
    )
        .into_response()
}

pub async fn logout() -> Response {
    let cookie = format!("{COOKIE_NAME}=; Path=/; MaxAge=-1; HttpOnly; SameSite=Lax");
    (
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, cookie.as_str()),
            (header::LOCATION, "/login"),
        ],
        "",
    )
        .into_response()
}

