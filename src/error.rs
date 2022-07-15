use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid mqtt client information")]
    ClientInformation,
    #[error("broker connection failed")]
    BrokerConnection,
    #[error("topic subscription failed")]
    Subscription,
    #[error("no message received before timeout")]
    PublishTimeout,
    #[error("message reception failed")]
    MessageReception,
    #[error("invalid message payload")]
    Payload,
    #[error("publish failed")]
    Publish,
    #[error("disconnection failure")]
    Disconnect,
    #[error("missing or invalid header")]
    Header,
    #[error("invalid broker url")]
    BrokerUrl,
    #[error("invalid json format or payload too large")]
    JsonFormat,
    #[error("body too large")]
    BodySize,
    #[error("invalid topic path")]
    Topic,
}

impl Error {
    fn status_code(&self) -> StatusCode {
        use Error::*;
        match self {
            ClientInformation => StatusCode::BAD_REQUEST,
            BrokerConnection => StatusCode::BAD_GATEWAY,
            Subscription => StatusCode::BAD_GATEWAY,
            PublishTimeout => StatusCode::GATEWAY_TIMEOUT,
            MessageReception => StatusCode::BAD_GATEWAY,
            Payload => StatusCode::BAD_REQUEST,
            Publish => StatusCode::BAD_GATEWAY,
            Disconnect => StatusCode::BAD_GATEWAY,
            Header => StatusCode::BAD_REQUEST,
            BrokerUrl => StatusCode::BAD_REQUEST,
            JsonFormat => StatusCode::BAD_REQUEST,
            BodySize => StatusCode::PAYLOAD_TOO_LARGE,
            Topic => StatusCode::BAD_REQUEST,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
