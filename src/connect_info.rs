use axum::{
    async_trait,
    body::Body,
    extract::{FromRequest, RequestParts},
    http::Uri,
};
use serde::Deserialize;
use url::Url;

use crate::{
    misc::{header_str, parse_url_with_default},
    Error,
};

pub struct ConnectInfo {
    pub broker: Url,
    pub credentials: Option<Credentials>,
}

#[async_trait]
impl FromRequest<Body> for ConnectInfo {
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<Body>) -> Result<Self, Self::Rejection> {
        Ok(Self {
            broker: parse_url_with_default(
                header_str(req.headers(), "X-Broker").ok_or(Error::Header)?,
            )
            .map_err(|_| Error::BrokerUrl)?,
            credentials: header_str(req.headers(), "X-Username").and_then(|username| {
                Some(Credentials {
                    username: username.to_owned(),
                    password: header_str(req.headers(), "X-Password")?.to_owned(),
                })
            }),
        })
    }
}

#[derive(Deserialize, PartialEq, Debug)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

pub struct Topic(pub String);

#[async_trait]
impl FromRequest<Body> for Topic {
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<Body>) -> Result<Self, Self::Rejection> {
        Ok(Self(
            Uri::from_request(req)
                .await
                .map_err(|_| Error::Topic)?
                .path()
                .trim_start_matches('/')
                .to_owned(),
        ))
    }
}
