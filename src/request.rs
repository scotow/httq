use paho_mqtt::QOS_2;
use serde::de::Unexpected;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use url::{ParseError, Url};

#[derive(Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum PublishRequest {
    Single(Publish),
    Multiple(Vec<Publish>),
}

impl IntoIterator for PublishRequest {
    type Item = Publish;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            PublishRequest::Single(p) => vec![p],
            PublishRequest::Multiple(ps) => ps,
        }
        .into_iter()
    }
}

#[derive(Deserialize, PartialEq, Debug)]
pub struct Publish {
    #[serde(
        alias = "host",
        alias = "hostname",
        deserialize_with = "Publish::deserialize_broker"
    )]
    pub broker: Url,
    #[serde(flatten)]
    pub credentials: Option<Credentials>,
    #[serde(flatten)]
    pub messages: MessagePublish,
}

impl Publish {
    fn deserialize_broker<'de, D>(deserializer: D) -> Result<Url, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        match input.parse() {
            Ok(url) => Ok(url),
            Err(ParseError::RelativeUrlWithoutBase) => {
                let with_tcp = format!("tcp://{}", input);
                with_tcp.parse::<Url>().map_err(|err| {
                    serde::de::Error::invalid_value(
                        Unexpected::Str(&input),
                        &err.to_string().as_str(),
                    )
                })
            }
            Err(err) => Err(serde::de::Error::invalid_value(
                Unexpected::Str(&input),
                &err.to_string().as_str(),
            )),
        }
    }
}

#[derive(Deserialize, PartialEq, Debug)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum MessagePublish {
    Single(Message),
    #[serde(alias = "message")]
    Multiple {
        messages: Vec<Message>,
    },
}

impl IntoIterator for MessagePublish {
    type Item = Message;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Single(m) => vec![m],
            Self::Multiple { messages: ms } => ms,
        }
        .into_iter()
    }
}

#[derive(PartialEq, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub topic: String,
    payload: Option<Value>,
    #[serde(default)]
    payload_type: PayloadType,
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
        Some(match (self.payload_type, payload) {
            (PayloadType::String, Value::String(s)) => s.into_bytes(),
            (PayloadType::String, Value::Number(n)) => n.to_string().into_bytes(),
            (PayloadType::Json, v) => v.to_string().into_bytes(),
            (PayloadType::Base64, Value::String(d)) => base64::decode(&d).ok()?,
            (_, Value::Null) => Vec::new(),
            _ => return None,
        })
    }
}

impl Default for Message {
    fn default() -> Self {
        Self {
            topic: "".to_owned(),
            payload: Default::default(),
            payload_type: Default::default(),
            qos: QOS_2,
        }
    }
}

#[derive(Deserialize, PartialEq, Default, Debug)]
#[serde(rename_all = "camelCase")]
enum PayloadType {
    #[default]
    String,
    Json,
    Base64,
}

#[cfg(test)]
mod tests {
    use super::{Credentials, Message, MessagePublish, PayloadType, Publish, PublishRequest};
    use serde_json::json;
    use serde_json::Value;

    fn json_req(json: Value) -> Option<PublishRequest> {
        serde_json::from_value(json).ok()
    }

    fn json_message(json: Value) -> Option<Message> {
        serde_json::from_value(json).ok()
    }

    mod deserialize {
        use super::*;

        #[test]
        fn single_simple() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "topic": "door",
                }))
                .unwrap(),
                PublishRequest::Single(Publish {
                    broker: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessagePublish::Single(Message {
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
                PublishRequest::Multiple(vec![Publish {
                    broker: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessagePublish::Single(Message {
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
                PublishRequest::Single(Publish {
                    broker: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessagePublish::Single(Message {
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
                PublishRequest::Single(Publish {
                    broker: "ws://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessagePublish::Single(Message {
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
                PublishRequest::Single(Publish {
                    broker: "tcp://broker.com".parse().unwrap(),
                    credentials: Some(Credentials {
                        username: "user_1".to_owned(),
                        password: "qwerty123".to_owned(),
                    }),
                    messages: MessagePublish::Single(Message {
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
                PublishRequest::Single(Publish {
                    broker: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessagePublish::Single(Message {
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
                PublishRequest::Single(Publish {
                    broker: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessagePublish::Single(Message {
                        topic: "door".to_owned(),
                        ..Default::default()
                    })
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
                            "payload": "open",
                        },
                        {
                            "topic": "light",
                            "payload": "off",
                        }
                    ]
                }))
                .unwrap(),
                PublishRequest::Single(Publish {
                    broker: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessagePublish::Multiple {
                        messages: vec![
                            Message {
                                topic: "door".to_owned(),
                                payload: Some("open".into()),
                                ..Default::default()
                            },
                            Message {
                                topic: "light".to_owned(),
                                payload: Some("off".into()),
                                ..Default::default()
                            }
                        ]
                    }
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

        #[test]
        fn base64() {
            assert_eq!(
                json_req(json!({
                    "hostname": "broker.com",
                    "topic": "door",
                    "payloadType": "base64",
                }))
                .unwrap(),
                PublishRequest::Single(Publish {
                    broker: "tcp://broker.com".parse().unwrap(),
                    credentials: None,
                    messages: MessagePublish::Single(Message {
                        topic: "door".to_owned(),
                        payload_type: PayloadType::Base64,
                        ..Default::default()
                    })
                })
            );
        }
    }

    mod payloads {
        use super::*;

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
        fn invalid_type() {
            assert!(json_message(json!({
                "topic": "door",
                "payloadType": "unknown",
            }))
            .is_none());
        }
    }
}
