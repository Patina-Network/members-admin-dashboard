# members-admin-dashboard

React UI served by a Rust HTTP backend. The backend calls the members admin
service through a generated gRPC client; it does not run a gRPC server or connect
directly to the members database.

## Local development

Prerequisites: Rust (CI/Docker use 1.99), Node 22, pnpm 10.24.0, and
`just`. Enable pnpm with `corepack enable`. Connect to the Patina VPN to download
`members-grpc-service` from the private Cargo registry and call staging.

```sh
just install       # install frontend dependencies from the lockfile
just run           # build frontend, then start Rust on :8080
```

For frontend hot reload, run `just frontend` in another terminal and open
http://localhost:5173. Vite forwards `/api` and `/version` to Rust on :8080.
The Rust server serves the built UI on http://localhost:8080, including direct
navigation to client-side routes. Run `just frontend-build` after UI edits when
using that URL instead of Vite.

`just run` selects the staging gRPC host with TLS. To use custom settings, build
the frontend and run Cargo directly:

```sh
just frontend-build
MEMBERS_ADMIN_SERVICE_GRPC_HOST=localhost:50051 cargo run --locked
```

The frontend build must exist before compiling Rust because `rust-embed` embeds
`frontend/dist`. Build output is ignored by Git. `just run`, `just backend-test`,
`just lint`, CI, and Docker arrange this build order. A fresh clone needs
`just install` first. Initial startup is lazy and does not call the upstream.

## Checks

```sh
just test          # frontend typecheck/format/lint/build/tests, then Rust tests
just lint          # frontend checks/build, Rust formatting and Clippy
just fmt           # format Rust and frontend
just docker-build  # production image, requires Docker and registry access
```

Individual commands: `just frontend-test`, `just backend-test`, and
`pnpm --dir frontend vt`. Use `RUSTUP_TOOLCHAIN=<installed-version>` to select a
compatible local Rust toolchain without changing CI's version. CI runs the
frontend checks before backend checks and image builds. Vitest is configured
to allow no test files while the frontend is a static scaffold; feature tickets
should add behavior tests.

## Frontend conventions

Follow CodeBloom's `STYLEGUIDE.md` frontend conventions, adapted to `frontend/`
instead of `js/`:

- Pages live in `src/app/<route>/*.page.tsx`, registered in `src/lib/router.tsx`.
- Components use PascalCase; ordinary TypeScript files/functions use camelCase.
  Keep route-local components in `_components` and reusable UI in `components`.
- Prefer Mantine components, their props, and `lib/theme.ts`; use `.module.css`
  for complex styles. Avoid hard-coded styles and production Tailwind additions.
- Put reusable queries/mutations in `lib/api/queries/<service>/index.ts`, outside
  components. Keep reusable hooks/types separate from UI.
- Use ESLint, Prettier, Stylelint, TypeScript, and Vitest. DOM test setup lives in
  `lib/test/setup.ts`; use Testing Library for component behavior tests.

The theme and animated Mantine wrappers follow CodeBloom. Existing login,
Database, and modal components are scaffolding: feature buttons are inert, there is no authentication state or route guard,
and no member operations are connected. Page navigation and visual interactions
remain available for preview. Real Azure authorization,
member CRUD, pagination/counts, and CSV import remain separate feature tickets.

## HTTP API types

Client generation runs automatically before frontend development, typechecking,
and builds, including CI. To regenerate explicitly:

```sh
pnpm --dir frontend generate
```

This compiles the `export-openapi` Rust binary and generates the Hey API client
from its local schema, without starting a server or contacting the upstream.
Rust and private Cargo registry access are required. Both `frontend/openapi.json`
and `frontend/src/lib/api/generated` are ignored by Git; do not edit or commit
these generated files. Docker exports the schema in a Rust stage before building
the frontend. The exporter creates an empty `frontend/dist` directory on a fresh
clone to satisfy `rust-embed`; the frontend build then fills it before the final
server build.

Swagger UI is available at `/api/swagger-ui/`, with the schema at
`/api/openapi.json`. Set `ENABLE_DOCUMENTATION=false` to disable both routes
(default: `true`). Offline client generation works regardless of this setting.

For a new endpoint, return an explicit enum deriving `utoipa::IntoResponses`
and implementing Axum's `IntoResponse`, and reference it with
`responses(MyResponse)` in `utoipa::path`. Use `VersionResponse` as the example.
Add success and failure variants for the states the endpoint actually returns,
and derive `utoipa::ToSchema` for body types. Register the handler in both
`src/http/openapi.rs` and the Axum router, then regenerate.
The enum shares payload types with the documentation and makes response matching
exhaustive. Status codes and serialization in `IntoResponse` still need to agree
with the annotations; Utoipa does not check that implementation automatically.
Only `/version` and `ErrorBody` are defined now.

The generated SDK and TanStack Query helpers live in that folder. Same-origin
`baseUrl` is set in `frontend/src/lib/api/hey-api.ts`. Put service hooks under
`lib/api/queries`. There are no frontend API calls in this scaffold yet.

## Rust conventions

Follow `hello-world-grpc-client-rust`: Axum handlers under `src/http`, Serde HTTP
request/response types, and the generated client in `AppState`. Use
`grpc::call` for outgoing calls so both the gRPC deadline and local timeout apply.
Use `UpstreamError` for consistent HTTP error responses and server-side logging.
Keep protobuf types at the gRPC boundary and convert them to HTTP response types.
Test controllers and repositories directly with unit tests; avoid full-router HTTP
tests for the scaffold.

The generated client is pinned to stable `members-grpc-service` 1.0.0 from the
Patina registry. Feature tickets must update it when they require a newer
published member API; no member RPCs are invoked by the setup scaffold.

## Environment

| Variable | Default | Purpose |
| --- | --- | --- |
| `MEMBERS_ADMIN_SERVICE_GRPC_HOST` | `members-admin-grpc-service:50051` | Upstream host and port |
| `MEMBERS_ADMIN_SERVICE_GRPC_TLS` | `false` | Enable TLS with native roots |
| `MEMBERS_ADMIN_SERVICE_GRPC_TIMEOUT_MS` | `3000` | Connection and request timeout |
| `ENABLE_DOCUMENTATION` | `true` | Serve Swagger UI and OpenAPI routes |
| `HTTP_PORT` | `8080` | HTTP port; binds `0.0.0.0` |
| `VERSION` | unset | `/version` returns this, or `N/A` |
| `ENVIRONMENT` | unset | `production`/`staging` enables JSON logs |
| `RUST_LOG` | `info` | Tracing filter |

Health endpoints `/livez` and `/readyz` currently report process availability,
not upstream gRPC availability. `/metrics` exposes process metrics. The server
handles SIGINT/SIGTERM for graceful shutdown. Docker builds the frontend before
embedding it in the release binary and runs as a non-root user.
