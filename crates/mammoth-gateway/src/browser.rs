//! Browser-origin protection for an unauthenticated local service. This is not
//! authentication: non-browser clients with network access still have full access.
use axum::{
    extract::Request,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Response},
};

fn local_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.trim_matches(['[', ']']).parse::<std::net::IpAddr>().is_ok()
}

fn valid_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else { return true };
    if headers.get_all(header::ORIGIN).iter().count() != 1 {
        return false;
    }
    let Some(origin) = origin.to_str().ok().and_then(|s| s.parse::<Uri>().ok()) else {
        return false;
    };
    let Some(host) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else { return false };
    let Some(authority) = origin.authority() else { return false };
    matches!(origin.scheme_str(), Some("http" | "https"))
        && origin.path() == "/"
        && origin.query().is_none()
        && local_host(authority.host())
        && authority.as_str().eq_ignore_ascii_case(host)
}

pub(crate) async fn guard(request: Request, next: Next) -> Response {
    let headers = request.headers();
    let site = headers.get("sec-fetch-site");
    let browser = site.is_some() || headers.contains_key(header::ORIGIN);
    // Reject DNS-rebinding hostnames for browser traffic. The supported browser
    // endpoints are localhost or literal IPs. Do not trust forwarded headers.
    let host_ok = !browser
        || headers
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.parse::<axum::http::uri::Authority>().ok())
            .is_some_and(|a| local_host(a.host()));
    let mutation = !matches!(*request.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    let site_ok = !mutation || site.is_none_or(|v| v == "same-origin" || v == "none");
    let mut response = if !host_ok || !site_ok || !valid_origin(headers) {
        (StatusCode::FORBIDDEN, "Browser request rejected: open Mammoth using its localhost or IP address and use the same origin.").into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert("cross-origin-resource-policy", HeaderValue::from_static("same-origin"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    response
}
