# Rust build environment 
FROM rust:bullseye as builder

WORKDIR /usr/src/neko_server

# Cache dependencies
RUN echo "fn main() {}" > dummy.rs
COPY Cargo.toml .
RUN sed -i 's#src/main.rs#dummy.rs#' Cargo.toml
RUN cargo build --release
RUN sed -i 's#dummy.rs#src/main.rs#' Cargo.toml

# Build
COPY . .
RUN cargo install --path .

# Run Server
FROM debian:bullseye-slim
EXPOSE 80

# Copy binary from builder
COPY --from=builder /usr/local/cargo/bin/neko_server /usr/local/bin

CMD ["neko_server"]