set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

run *args: frontend-build
    MEMBERS_ADMIN_SERVICE_GRPC_HOST=stg.members-admin-grpc-service.vpn.patinanetwork.org:50051 \
    MEMBERS_ADMIN_SERVICE_GRPC_TLS=true \
    HTTP_PORT=8080 \
    cargo run {{ args }}

frontend *args:
    pnpm --dir frontend dev {{ args }}

test: frontend-test backend-test

backend-test *args: frontend-build
    cargo test --locked {{ args }}

lint:
    pnpm --dir frontend typecheck
    pnpm --dir frontend prettier
    pnpm --dir frontend lint
    pnpm --dir frontend build
    cargo fmt --check
    cargo clippy --locked --all-targets -- -D warnings

fmt:
    cargo fmt
    pnpm --dir frontend prettier:fix

docker-build tag="members-admin-dashboard":
    docker build -t {{ tag }} .

install:
    pnpm --dir frontend install --frozen-lockfile

frontend-build:
    pnpm --dir frontend build

frontend-test:
    pnpm --dir frontend test
