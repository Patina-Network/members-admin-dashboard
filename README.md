# hello-world-grpc-client-rust

Hello World gRPC Rust client for Patina Network

The UI is a single `frontend/index.html` (Tailwind from a CDN, no build step), embedded into the binary at compile time with `include_str!`.

## Prerequisites

- Rust 1.99 via rustup [(brew.sh)](https://formulae.brew.sh/formula/rustup)
- just [(brew.sh)](https://formulae.brew.sh/formula/just)
- Docker, for `just docker-build` [(brew.sh)](https://formulae.brew.sh/cask/docker-desktop)
- Tailscale, connected to the Patina VPN [(brew.sh)](https://formulae.brew.sh/cask/tailscale-app)

> [!NOTE]
> You must be connected to the VPN to connect locally. You can find the instructions to connect at <https://docs.patinanetwork.org/infra/how-to-connect-to-vpn/>

The VPN is needed both to download the `hello-world-grpc-service` crate from the private Cargo registry (`pkg.vpn.patinanetwork.org`) and to reach the staging gRPC service.

## Development

```sh
just run            # run the backend + UI on :8080 against the staging gRPC service
just test
just lint           # cargo fmt --check + clippy
just fmt
just docker-build
```

## Environment

| Variable                              | Default                          | Description                                                                        |
| ------------------------------------- | -------------------------------- | ---------------------------------------------------------------------------------- |
| `HELLO_WORLD_SERVICE_GRPC_HOST`       | `hello-world-grpc-service:50051` | `host:port` of the gRPC service                                                    |
| `HELLO_WORLD_SERVICE_GRPC_TLS`        | `false`                          | Connect to the gRPC service over TLS (`true`/`false`)                              |
| `HELLO_WORLD_SERVICE_GRPC_TIMEOUT_MS` | `3000`                           | Deadline for each gRPC call, in milliseconds                                       |
| `HTTP_PORT`                           | `8080`                           | Port the HTTP server listens on (always binds `0.0.0.0`)                           |
| `VERSION`                             | unset                            | Returned by `GET /version`; `N/A` when unset                                       |
| `HELLO_WORLD_CLIENT_URLS`             | unset                            | Comma-separated URLs returned by `GET /urls` and listed in the UI                  |
| `ENVIRONMENT`                         | unset                            | `production` or `staging` switches logs to JSON; anything else logs human-readable |
| `RUST_LOG`                            | `info`                           | Log filter, e.g. `debug` or `hello_world_client_rust=debug`                        |

Values that don't parse (e.g. `HTTP_PORT=http`) fall back to the default. `just run` sets `HELLO_WORLD_SERVICE_GRPC_HOST` to the staging service and `HELLO_WORLD_SERVICE_GRPC_TLS=true`.

## Releases

On `main`, CI creates an unprefixed semantic-version tag after both image builds succeed, using GitHub App credentials. Tags start at `1.0.0` and increment the patch version; source package versions are unchanged.

The tag triggers CD. Because the tagging step creates a new commit, CD promotes images built from its parent commit to the release tag and `latest`, then deploys production using the release tag. CI deploys only staging; production deployment runs only in CD.

Staging and production Kubernetes deployments use `patinanetwork/hello-world-client-rust-arm`. Manifest directories remain `base/<environment>/hello-world-client-rust`.
