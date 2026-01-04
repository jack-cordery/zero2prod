FROM rust:1.6.7

WORKDIR /app 

RUN apt update && apt install clang lld -y

COPY . . 

RUN cargo build --release

ENTRYPOINT ["./target/release/zero2prod"]


