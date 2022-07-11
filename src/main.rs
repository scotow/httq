use std::{error::Error, net::SocketAddr, time::Duration};

use axum::{
    http::{header, HeaderMap, StatusCode},
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
    misc::header_str,
    publish::PublishRequest,
};

mod connect_info;
mod misc;
mod publish;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
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

async fn publish_handler(req: PublishRequest) -> Result<StatusCode, StatusCode> {
    for broker in req {
        let client = AsyncClient::new(
            CreateOptionsBuilder::new()
                .server_uri(broker.url)
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

async fn subscribe_handler(
    connect_info: ConnectInfo,
    Topic(topic): Topic,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let mut client = AsyncClient::new(
        CreateOptionsBuilder::new()
            .server_uri(connect_info.broker)
            .finalize(),
    )
    .map_err(|_| StatusCode::BAD_GATEWAY)?;

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
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    client
        .subscribe(topic, QOS_2)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let message = timeout(Duration::from_secs(5 * 60), stream.next())
        .await
        .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
        .flatten()
        .ok_or(StatusCode::BAD_GATEWAY)?;

    Ok(
        if header_str(&headers, header::ACCEPT) == Some("text/plain") {
            (
                [(header::CONTENT_TYPE, "text/plain")],
                message.payload_str().into_owned(),
            )
                .into_response()
        } else {
            message.payload().to_vec().into_response()
        },
    )
}
