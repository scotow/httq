use axum::http::{header::AsHeaderName, HeaderMap};
use url::{ParseError as UrlParseError, Url};

pub fn header_str<H: AsHeaderName>(headers: &HeaderMap, name: H) -> Option<&str> {
    headers.get(name)?.to_str().ok()
}

pub fn parse_url_with_default(input: &str) -> Result<Url, UrlParseError> {
    match input.parse() {
        Ok(url) => Ok(url),
        Err(UrlParseError::RelativeUrlWithoutBase) => format!("tcp://{}", input).parse(),
        Err(err) => Err(err),
    }
}
