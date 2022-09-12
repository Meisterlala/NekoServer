FROM rust:1.63-slim-buster as builder

WORKDIR /usr/src/neko_server
COPY . .

RUN echo "fn main() {}" > dummy.rs
COPY Cargo.toml .
RUN sed -i 's#src/main.rs#dummy.rs#' Cargo.toml

RUN cargo build --release
RUN sed -i 's#dummy.rs#src/main.rs#' Cargo.toml

RUN cargo install --path .

FROM debian:buster-slim

EXPOSE 8080
COPY --from=builder /usr/local/cargo/bin/neko_server /usr/local/bin/neko_server

CMD ["neko_server", "--db", "./db.sqlite", "--port" "8080"]