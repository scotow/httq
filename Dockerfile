FROM rust:1.62-slim AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

#------------

FROM gcr.io/distroless/cc

COPY --from=builder /app/target/release/httq /httq

ENTRYPOINT ["/httq"]