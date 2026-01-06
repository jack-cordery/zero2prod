FROM rust:1.92 AS builder
WORKDIR /usr/src/myapp
COPY . .
RUN apt update && apt install lld clang -y
ENV SQLX_OFFLINE=true
RUN cargo install --path .

FROM rust:1.92-slim
RUN apt-get update && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/zero2prod /usr/local/bin/zero2prod
COPY configuration configuration
ENV APP_ENVIRONMENT=production
CMD ["zero2prod"]
EXPOSE 8000
