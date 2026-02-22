FROM rust:1.93-bookworm AS builder
WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY shared/ shared/
COPY scheduling-service/ scheduling-service/

# Other services
COPY data-service/ data-service/
RUN mkdir -p data-service/src && echo "fn main() {}" > data-service/src/main.rs

COPY .sqlx/ .sqlx/
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin scheduling-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/scheduling-service /usr/local/bin/
COPY scheduling-service/scheduling.toml /etc/scheduling-service/scheduling.toml
ENV SCHEDULING_CONFIG_PATH=/etc/scheduling-service/scheduling.toml
CMD [ "scheduling-service" ]
