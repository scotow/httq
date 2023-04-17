use axum::{
    async_trait,
    body::{Body, Bytes},
    extract::{ContentLengthLimit, FromRequest, RequestParts},
    http::header,
    Json,
};
use base64::engine::{general_purpose::STANDARD as BASE64, Engine as _};
use paho_mqtt::QOS_2;
use serde::{de::Unexpected, Deserialize, Deserializer};
use serde_json::Value;
use url::Url;

use crate::{
    connect_info::{ConnectInfo, Credentials, Topic},
    misc::{header_str, parse_url_with_default},
    Error,
};

const MAX_PAYLOAD_SIZE: u64 = 16_777_216;

#[derive(Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum PublishRequest {
    Single(Broker),
    Multiple(Vec<Broker>),
}

impl IntoIterator for PublishRequest {
    type Item = Broker;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            PublishRequest::Single(p) => vec![p],
            PublishRequest::Multiple(ps) => ps,
        }
        .into_iter()
    }
}

#[async_trait]
impl FromRequest<Body> for PublishRequest {
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<Body>) -> Result<Self, Self::Rejection> {
        if header_str(req.headers(), header::CONTENT_TYPE) == Some("application/json") {
            ContentLengthLimit::<Json<PublishRequest>, MAX_PAYLOAD_SIZE>::from_request(req)
                .await
                .map_err(|_| Error::JsonFormat)
                .map(|data| data.0 .0)
        } else {
            let ConnectInfo {
                broker,
                credentials,
            } = ConnectInfo::from_request(req).await?;
            let Topic(topic) = Topic::from_request(req).await?;
            let ContentLengthLimit(payload) =
                ContentLengthLimit::<Bytes, MAX_PAYLOAD_SIZE>::from_request(req)
                    .await
                    .map_err(|_| Error::BodySize)?;
            Ok(Self::Single(Broker {
                url: broker,
                credentials,
                messages: MessageGroup::Flat(Message {
                    topic,
                    payload: Some(Payload::Specified(TypedPayload::Raw(payload.to_vec()))),
                    qos: QOS_2,
                }),
            }))
        }
    }
}

#[derive(Deserialize, PartialEq, Debug)]
pub struct Broker {
    #[serde(
        alias = "broker",
        alias = "host",
        alias = "hostname",
        deserialize_with = "Broker::deserialize_url"
    )]
    pub url: Url,
    #[serde(flatten)]
    pub credentials: Option<Credentials>,
    #[serde(flatten)]
    pub messages: MessageGroup,
}

impl Broker {
    fn deserialize_url<'de, D>(deserializer: D) -> Result<Url, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        parse_url_with_default(&input).map_err(|err| {
            serde::de::Error::invalid_value(Unexpected::Str(&input), &err.to_string().as_str())
        })
    }
}

#[derive(Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum MessageGroup {
    Flat(Message),
    Single {
        message: Message,
    },
    Multiple {
        #[serde(alias = "message")]
        messages: Vec<Message>,
    },
}

impl IntoIterator for MessageGroup {
    type Item = Message;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Flat(m) => vec![m],
            Self::Single { message: m } => vec![m],
            Self::Multiple { messages: ms } => ms,
        }
        .into_iter()
    }
}

#[derive(PartialEq, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub topic: String,
    #[serde(flatten)]
    payload: Option<Payload>,
    #[serde(
        default = "Message::default_qos",
        deserialize_with = "Message::deserialize_qos"
    )]
    pub qos: i32,
}

impl Message {
    fn default_qos() -> i32 {
        QOS_2
    }

    fn deserialize_qos<'de, D>(deserializer: D) -> Result<i32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let qos = i32::deserialize(deserializer)?;
        if (0..=2).contains(&qos) {
            Ok(qos)
        } else {
            Err(serde::de::Error::invalid_value(
                Unexpected::Signed(qos as i64),
                &"QOS between 0 and 2",
            ))
        }
    }

    pub fn payload(self) -> Option<Vec<u8>> {
        let payload = match self.payload {
            Some(payload) => payload,
            None => return Some(Vec::new()),
        };
        Some(match payload {
            Payload::Specified(TypedPayload::String(s)) => s.into_bytes(),
            Payload::Specified(TypedPayload::Json(v)) => v.to_string().into_bytes(),
            Payload::Specified(TypedPayload::Base64(d)) => BASE64.decode(&d).ok()?,
            Payload::Specified(TypedPayload::Raw(d)) => d,
            Payload::Unspecified { payload: s } => s.into_bytes(),
        })
    }
}

impl Default for Message {
    fn default() -> Self {
        Self {
            topic: Default::default(),
            payload: Default::default(),
            qos: QOS_2,
        }
    }
}

#[derive(Deserialize, PartialEq, Debug)]
#[serde(untagged)]
enum Payload {
    Specified(TypedPayload), // Must be first, or it will match Unspecified every time.
    Unspecified { payload: String },
}

#[derive(Deserialize, PartialEq, Debug)]
#[serde(tag = "payloadType", content = "payload", rename_all = "camelCase")]
enum TypedPayload {
    String(String),
    Json(Value),
    Base64(String),
    Raw(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{Broker, Credentials, Message, MessageGroup, PublishRequest};

    fn json_req(json: Value) -> Option<PublishRequest> {
        serde_json::from_value(json).ok()
    }

    fn json_message(json: Value) -> Option<Message> {
        serde_json::from_value(json).ok()
    }

    mod deserialize {
        use super::*;
        use crate::publish::{Payload, TypedPayload};

        #[test]
        fn single_simple() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "topic": "door",
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Flat(Message {
                        topic: "door".to_owned(),
                        ..Default::default()
                    })
                })
            );
        }

        #[test]
        fn multiple_simple() {
            assert_eq!(
                json_req(json!([
                    {
                        "hostname": "broker.com",
                        "topic": "door",
                    }
                ]))
                .unwrap(),
                PublishRequest::Multiple(vec![Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Flat(Message {
                        topic: "door".to_owned(),
                        ..Default::default()
                    })
                }])
            );
        }

        #[test]
        fn protocol_overwrite() {
            assert_eq!(
                json_req(json!({
                    "hostname": "tcp://broker.com",
                    "topic": "door",
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Flat(Message {
                        topic: "door".to_owned(),
                        ..Default::default()
                    })
                })
            );
        }

        #[test]
        fn protocol_ws_overwrite() {
            assert_eq!(
                json_req(json!({
                    "hostname": "ws://broker.com",
                    "topic": "door",
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "ws://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Flat(Message {
                        topic: "door".to_owned(),
                        ..Default::default()
                    })
                })
            );
        }

        #[test]
        fn credentials() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "username": "user_1",
                    "password": "qwerty123",
                    "topic": "door",
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: Some(Credentials {
                        username: "user_1".to_owned(),
                        password: "qwerty123".to_owned(),
                    }),
                    messages: MessageGroup::Flat(Message {
                        topic: "door".to_owned(),
                        ..Default::default()
                    })
                })
            );
        }

        #[test]
        fn missing_username() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "password": "qwerty123",
                    "topic": "door",
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Flat(Message {
                        topic: "door".to_owned(),
                        ..Default::default()
                    })
                })
            );
        }

        #[test]
        fn missing_password() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "username": "user_1",
                    "topic": "door",
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Flat(Message {
                        topic: "door".to_owned(),
                        ..Default::default()
                    })
                })
            );
        }

        #[test]
        fn message_object() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "username": "user_1",
                    "message": {
                        "topic": "door",
                    }
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Single {
                        message: Message {
                            topic: "door".to_owned(),
                            ..Default::default()
                        }
                    }
                })
            );
        }

        #[test]
        fn message_array() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "username": "user_1",
                    "messages": [
                        {
                            "topic": "door",
                        },
                        {
                            "topic": "light",
                        }
                    ]
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Multiple {
                        messages: vec![
                            Message {
                                topic: "door".to_owned(),
                                ..Default::default()
                            },
                            Message {
                                topic: "light".to_owned(),
                                ..Default::default()
                            }
                        ]
                    }
                })
            );
        }

        #[test]
        fn payload_untyped() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "topic": "door",
                    "payload": "open",
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Flat(Message {
                        topic: "door".to_owned(),
                        payload: Some(Payload::Unspecified {
                            payload: "open".to_owned(),
                        }),
                        ..Default::default()
                    })
                })
            );
        }

        #[test]
        fn payload_typed() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "topic": "door",
                    "payload": "open",
                    "payloadType": "string",
                }))
                .unwrap(),
                PublishRequest::Single(Broker {
                    url: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessageGroup::Flat(Message {
                        topic: "door".to_owned(),
                        payload: Some(Payload::Specified(TypedPayload::String("open".to_owned()))),
                        ..Default::default()
                    })
                })
            );
        }

        #[test]
        fn invalid_qos() {
            assert!(json_req(json!({
                "hostname": "broker.com",
                "topic": "door",
                "qos": 3,
            }))
            .is_none());
        }
    }

    mod payloads {
        use super::*;

        #[test]
        fn none() {
            assert_eq!(
                json_message(json!({
                    "topic": "door",
                }))
                .unwrap()
                .payload(),
                Some(Vec::new())
            );
        }

        #[test]
        fn auto_string() {
            assert_eq!(
                json_message(json!({
                    "topic": "door",
                    "payload": "open",
                }))
                .unwrap()
                .payload()
                .as_deref(),
                Some("open".as_bytes())
            );
        }

        #[test]
        fn string() {
            assert_eq!(
                json_message(json!({
                    "topic": "door",
                    "payloadType": "string",
                    "payload": "open",
                }))
                .unwrap()
                .payload()
                .as_deref(),
                Some("open".as_bytes())
            );
        }

        #[test]
        fn json() {
            assert_eq!(
                json_message(json!({
                    "topic": "door",
                    "payloadType": "json",
                    "payload": {
                        "door": 2,
                        "open": true,
                    },
                }))
                .unwrap()
                .payload()
                .as_deref(),
                Some(
                    json!({
                        "door": 2,
                        "open": true,
                    })
                    .to_string()
                    .as_bytes()
                )
            );
        }

        #[test]
        fn base64() {
            assert_eq!(
                json_message(json!({
                    "topic": "door",
                    "payloadType": "base64",
                    "payload": "AAEC",
                }))
                .unwrap()
                .payload(),
                Some(vec![0, 1, 2])
            );
        }

        #[test]
        fn default_to_string() {
            assert_eq!(
                json_message(json!({
                    "topic": "door",
                    "payloadType": "unknown",
                    "payload": "open"
                }))
                .unwrap()
                .payload()
                .as_deref(),
                Some("open".as_bytes())
            );
        }
    }
}
