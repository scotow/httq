use std::{error::Error as StdError, net::SocketAddr, time::Duration};

use axum::{
    http::{header, header::HeaderName, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router, Server,
};
use futures_util::StreamExt;
use paho_mqtt::{
    AsyncClient, ConnectOptions, ConnectOptionsBuilder, CreateOptionsBuilder, Message, QOS_2,
};
use tokio::time::timeout;

use crate::{
    connect_info::{ConnectInfo, Credentials, Topic},
    error::Error,
    misc::header_str,
    publish::PublishRequest,
};

mod connect_info;
mod error;
mod misc;
mod publish;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn StdError + Send + Sync>> {
    Server::bind(&SocketAddr::new("0.0.0.0".parse()?, 8080))
        .http1_title_case_headers(true)
        .serve(
            Router::new()
                .route("/*topic", post(publish_handler).get(subscribe_handler))
                .into_make_service(),
        )
        .await?;
    Ok(())
}

async fn publish_handler(req: PublishRequest) -> Result<StatusCode, Error> {
    for broker in req {
        let client = AsyncClient::new(
            CreateOptionsBuilder::new()
                .server_uri(broker.url)
                .finalize(),
        )
        .map_err(|_| Error::ClientInformation)?;

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
            .map_err(|_| Error::BrokerConnection)?;

        for message in broker.messages.into_iter() {
            let (topic, qos) = (message.topic.clone(), message.qos);
            let msg = Message::new(topic, message.payload().ok_or(Error::Payload)?, qos);
            client.publish(msg).await.map_err(|_| Error::Publish)?;
        }

        client
            .disconnect(None)
            .await
            .map_err(|_| Error::Disconnect)?;
    }

    Ok(StatusCode::OK)
}

async fn subscribe_handler(
    connect_info: ConnectInfo,
    Topic(topic): Topic,
    headers: HeaderMap,
) -> Result<Response, Error> {
    let mut client = AsyncClient::new(
        CreateOptionsBuilder::new()
            .server_uri(connect_info.broker)
            .finalize(),
    )
    .map_err(|_| Error::ClientInformation)?;

    let opts = match connect_info.credentials {
        Some(Credentials { username, password }) => ConnectOptionsBuilder::new()
            .user_name(username)
            .password(password)
            .finalize(),
        None => ConnectOptions::new(),
    };

    let mut stream = client.get_stream(1);
    client
        .connect(opts)
        .await
        .map_err(|_| Error::BrokerConnection)?;
    client
        .subscribe(topic, QOS_2)
        .await
        .map_err(|_| Error::Subscription)?;

    let message = timeout(Duration::from_secs(5 * 60), stream.next())
        .await
        .map_err(|_| Error::PublishTimeout)?
        .flatten()
        .ok_or(Error::MessageReception)?;

    client
        .disconnect(None)
        .await
        .map_err(|_| Error::Disconnect)?;

    Ok(
        if header_str(&headers, header::ACCEPT) == Some("text/plain") {
            (
                [
                    (header::CONTENT_TYPE, "text/plain"),
                    (HeaderName::from_static("x-topic"), message.topic()),
                ],
                message.payload_str().into_owned(),
            )
                .into_response()
        } else {
            (
                [(HeaderName::from_static("x-topic"), message.topic())],
                message.payload().to_vec(),
            )
                .into_response()
        },
    )
}
