set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

run *args:
    HELLO_WORLD_SERVICE_GRPC_HOST=stg.hello-world-grpc-service.vpn.patinanetwork.org:50051 \
    HELLO_WORLD_SERVICE_GRPC_TLS=true \
    HTTP_PORT=8082 \
    HELLO_WORLD_CLIENT_URLS=http://localhost:8080,http://localhost:8081 \
    cargo run {{ args }}

test *args:
    cargo test --locked {{ args }}

lint:
    cargo fmt --check
    cargo clippy --locked --all-targets -- -D warnings

fmt:
    cargo fmt

docker-build tag="hello-world-grpc-client-rust":
    docker build -t {{ tag }} .
