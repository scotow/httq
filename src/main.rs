use crate::request::{Credentials, PublishRequest};
use axum::extract::ContentLengthLimit;
use axum::http::StatusCode;
use axum::{routing::post, Json, Router, Server};
use paho_mqtt::{
    AsyncClient, ConnectOptions, ConnectOptionsBuilder, CreateOptionsBuilder, Message,
};
use std::error::Error;
use std::net::SocketAddr;

mod request;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    Server::bind(&SocketAddr::new("0.0.0.0".parse()?, 8080))
        .http1_title_case_headers(true)
        .serve(
            Router::new()
                .route("/", post(publish_handler))
                .into_make_service(),
        )
        .await?;
    Ok(())
}

async fn publish_handler(
    ContentLengthLimit(Json(req)): ContentLengthLimit<Json<PublishRequest>, 16_777_216>,
) -> Result<StatusCode, StatusCode> {
    for broker in req {
        let client = AsyncClient::new(
            CreateOptionsBuilder::new()
                .server_uri(broker.broker)
                .finalize(),
        )
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

        let opts = match broker.credentials {
            Some(Credentials { username, password }) => ConnectOptionsBuilder::new()
                .user_name(username)
                .password(password)
                .finalize(),
            None => ConnectOptions::new(),
        };
        client
            .connect(opts)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;

        for message in broker.messages.into_iter() {
            let (topic, qos) = (message.topic.clone(), message.qos);
            let msg = Message::new(
                topic,
                message.payload().ok_or(StatusCode::BAD_REQUEST)?,
                qos,
            );
            client
                .publish(msg)
                .await
                .map_err(|_| StatusCode::BAD_GATEWAY)?;
        }

        client
            .disconnect(None)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
    }

    Ok(StatusCode::OK)
}
