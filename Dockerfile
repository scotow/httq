FROM rust:1.68-slim AS builder

RUN apt update && apt install -y cmake

WORKDIR /app
COPY . .
RUN cargo build --release

#------------

FROM gcr.io/distroless/cc

COPY --from=builder /app/target/release/httq /httq

ENTRYPOINT ["/httq"]