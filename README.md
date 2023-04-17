# HTTQ

📬 A HTTP to MQTT proxy 📬 

## JSON Publish

Supported JSON format (all formats bellow are valid, some just use the default values):

### Broker URL (protocol, hostname and port):

```json
{
  "broker": "broker.com",
  "topic": "door"
}
```

```json
{
  "broker": "ws://broker.com",
  "topic": "door"
}
```

```json
{
  "broker": "tcp://broker.com:2222",
  "topic": "door"
}
```

### Credentials:

```json
{
  "broker": "broker.com",
  "username": "user1",
  "password": "qwerty",
  "topic": "door"
}
```

### QOS

```json
{
  "broker": "broker.com",
  "topic": "door",
  "qos": 1
}
```

### Payload

```json
{
  "broker": "broker.com",
  "topic": "door",
  "payload": "open"
}
```

```json
{
  "broker": "broker.com",
  "topic": "door",
  "payloadType": "string",
  "payload": "open"
}
```

```json
{
  "broker": "broker.com",
  "topic": "door",
  "payloadType": "base64",
  "payload": "AAEC"
}
```

```json
{
  "broker": "broker.com",
  "topic": "door",
  "payloadType": "json",
  "payload": {
    "doorNumber": 1,
    "state": "open"
  }
}
```

### Message field

```json
{
  "broker": "broker.com",
  "message": {
    "topic": "door",
    "payload": "open"
  }
}
```

```json
{
  "broker": "broker.com",
  "messages": [
    {
      "topic": "door",
      "payload": "open"
    },
    {
      "topic": "light"
    }
  ]
}
```

## HTTP headers + Body Publish

Only one message can be sent per request:

```sh
curl -H 'X-Broker: broker.com' -H 'X-Username: user1' -H 'X-Password: qwerty' --data-raw "open" localhost:8080/door
```

is equivalent to:

```json
{
  "broker": "broker.com",
  "username": "user1",
  "password": "qwerty",
  "topic": "door",
  "payload": "open"
}
```

## HTTP headers + Subscribe

Only one message can be received per request:

```sh
curl -X GET -H 'X-Broker: broker.com' -H 'X-Username: user1' -H 'X-Password: qwerty' -H 'Accept: text/plain' localhost:8080/door
```

will wait up to 5 min for a message on the `door` topic and will return the payload in the response's body.

Specifying `Accept: plain/text` will cast / force the message's payload to be cast to a string, discarding invalid UTF-8 parts. 

## Limitations

- No TLS/SSL broker connection support
- No multi-level wildcard subscribe (#)