# syntax=docker/dockerfile:1.7
FROM rust:1.99-bookworm AS rust-build
WORKDIR /src/app
COPY .cargo/config.toml .cargo/config.toml
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY frontend/ frontend/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/app/target \
    cargo build --locked --release && cp target/release/hello-world-client-rust /server

FROM gcr.io/distroless/cc-debian12:nonroot AS rust
WORKDIR /app
COPY --from=rust-build /server /app/server
ENV HTTP_PORT=8080
EXPOSE 8080
USER 65532:65532
ENTRYPOINT ["/app/server"]
