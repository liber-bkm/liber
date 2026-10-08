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

#[utoipa::path(
    get,
    path = "/login",
    responses(
        (status = 200, description = "Login form page", content_type = "text/html"),
    )
)]
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

#[utoipa::path(
    post,
    path = "/login",
    request_body(content = Object, description = "Token form as {token, next?}"),
    responses(
        (status = 303, description = "Cookie set, redirects onward"),
        (status = 401, description = "Wrong token", body = Object),
    )
)]
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

#[utoipa::path(
    get,
    path = "/logout",
    responses(
        (status = 303, description = "Cookie cleared, redirects to login (GET and POST)"),
    )
)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::{build_router, AppState};
    use liber_core::auth::auth_mac;
    use liber_core::store::Config;

    fn test_app(token: &str) -> (axum::Router, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config {
            base_dir: dir.path().to_path_buf(),
            device_id: "test-device".to_string(),
            ..Default::default()
        };
        let state = AppState::new(cfg, token.to_string());
        (build_router(state), dir)
    }

    fn get(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn open_server_needs_nothing() {
        let (app, _dir) = test_app("");
        let res = app.oneshot(get("/api/v2/bookmarks")).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn api_requires_bearer_without_cookie() {
        let (app, _dir) = test_app("tok");
        let res = app.oneshot(get("/api/v2/bookmarks")).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn bearer_grants_access() {
        let (app, _dir) = test_app("tok");
        let bearer = format!("Bearer {}", auth_mac("tok", "liber-bearer-v1"));
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks")
                    .header(header::AUTHORIZATION, bearer)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn login_form_issues_cookie() {
        let (app, _dir) = test_app("tok");
        let bad = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/login")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("token=nope"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(bad.status(), StatusCode::UNAUTHORIZED);

        let good = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/login")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from("token=tok"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(good.status(), StatusCode::SEE_OTHER);
        let set_cookie = good
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(set_cookie.starts_with(&format!("{COOKIE_NAME}=")));

        let cookie_pair = set_cookie.split(';').next().unwrap().to_string();
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/v2/bookmarks")
                    .header(header::COOKIE, cookie_pair)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn cookie_post_checks_origin() {
        let (app, _dir) = test_app("tok");
        let cookie = format!("{COOKIE_NAME}={}", auth_mac("tok", "liber-cookie-v1"));
        let evil = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v2/bookmarks")
                    .header(header::COOKIE, cookie.clone())
                    .header(header::ORIGIN, "http://evil.com")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"url\":\"https://example.com/x\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(evil.status(), StatusCode::FORBIDDEN);

        let same = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v2/bookmarks")
                    .header(header::COOKIE, cookie)
                    .header(header::ORIGIN, "http://127.0.0.1:8080")
                    .header(header::HOST, "127.0.0.1:8080")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{\"url\":\"https://example.com/x\"}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(same.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn pages_redirect_to_login() {
        let (app, _dir) = test_app("tok");
        let res = app.oneshot(get("/settings")).await.unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let _ = to_bytes(Body::empty(), 0).await;
    }
}
