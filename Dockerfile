FROM rust:latest

WORKDIR /app 

RUN apt update && apt install clang lld -y

COPY . . 

ENV SQLX_OFFLINE=true
ENV APP_ENVIRONMENT=production

RUN cargo build --release

ENTRYPOINT ["./target/release/zero2prod"]

EXPOSE 8000
