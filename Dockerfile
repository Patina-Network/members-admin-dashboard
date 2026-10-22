# syntax=docker/dockerfile:1.7
FROM rust:1.99-bookworm AS schema
WORKDIR /src/app
COPY .cargo/config.toml .cargo/config.toml
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN mkdir -p frontend/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/app/target \
    cargo run --locked --bin export-openapi > /openapi.json

FROM node:22-bookworm AS frontend
WORKDIR /src/app/frontend
COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile
COPY frontend/ ./
COPY --from=schema /openapi.json ./openapi.json
RUN pnpm exec openapi-ts && pnpm exec vite build

FROM rust:1.99-bookworm AS rust-build
WORKDIR /src/app
COPY .cargo/config.toml .cargo/config.toml
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY --from=frontend /src/app/frontend/dist frontend/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/app/target \
    cargo build --locked --release --bin members-admin-dashboard && cp target/release/members-admin-dashboard /server

FROM gcr.io/distroless/cc-debian12:nonroot AS rust
WORKDIR /app
COPY --from=rust-build /server /app/server
ENV HTTP_PORT=8080
EXPOSE 8080
USER 65532:65532
ENTRYPOINT ["/app/server"]
