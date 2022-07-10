use axum::extract::RequestParts;
use axum::http::header::AsHeaderName;

pub fn header_str<B, H: AsHeaderName>(req: &RequestParts<B>, name: H) -> Option<&str> {
    req.headers().get(name)?.to_str().ok()
}
