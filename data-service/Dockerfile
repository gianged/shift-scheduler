FROM rust:1.93-bookworm AS builder
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY shared/ shared/
COPY data-service/ data-service/

# Other services
COPY scheduling-service/ scheduling-service/
RUN mkdir -p scheduling-service/src && echo "fn main() {}" > scheduling-service/src/main.rs

COPY .sqlx/ .sqlx/
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin data-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/data-service /usr/local/bin/
CMD [ "data-service" ]
