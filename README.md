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
`just install` first. The gRPC channel is lazy; startup does not call the member service. Azure metadata
is fetched during sign-in.

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
frontend checks before backend checks and image builds. Vitest runs component behavior tests for authentication and Database access.

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

The theme, login card, and Home/Database navigation follow CodeBloom. Home has
no page content. Login and logout use the server session; Database requires the
configured Microsoft sign-in email allowlist. Member CRUD, pagination/counts, and CSV import remain
separate feature tickets; the Database table is still a scaffold.

## Azure authentication

Azure discovery and signing keys are cached in process for one hour and refreshed
on the next sign-in after expiry. Concurrent requests share one refresh. Failed
discovery attempts back off for 30 seconds. An unknown signing key triggers one
refresh and revalidation of the same ID token (without redeeming the code again),
limited to once every five minutes. Expired metadata or unresolved signing keys
deny sign-in.

The Rust backend uses the authorization-code flow with PKCE and OpenID Connect.
It validates Azure's signature, issuer, audience, expiration, nonce, tenant, and
callback state before creating a session. Azure tokens and the client secret
stay on the backend. Admin access uses `AZURE_ADMIN_EMAILS`, a comma-separated list of Microsoft
sign-in email addresses. Entries are trimmed and matched case-insensitively
against the validated ID token's standard `preferred_username` claim. Matching
is exact: no domain-wide access, wildcards, contact-email fallback, or Azure app
roles. Missing/empty configuration denies everyone. An account outside the list
or without a sign-in username does not receive a signed-in session.

For example: `AZURE_ADMIN_EMAILS=alice@example.com,bob@example.com`.
Use each person's Microsoft sign-in address, which can differ from their contact
email. The tenant restriction and all token validation checks still apply.
Session identity remains the token's `sub`, not the email address. This is the
MVP email policy: Microsoft usernames are mutable, so maintain the allowlist when
accounts are renamed or reassigned. A future database-backed access policy should
use immutable user IDs. See [Microsoft's claim reference](https://learn.microsoft.com/en-us/entra/identity-platform/id-token-claims-reference).

Configure the app registration through the platform-infra Azure app workflow:

1. Register a single-tenant **Web** application, with a redirect URI of
   `<APP_ORIGIN>/oauth2/callback`. For Vite development use
   `http://localhost:5173/oauth2/callback`; for the built app use
   `http://localhost:8080/oauth2/callback`.
2. Set `AZURE_ADMIN_EMAILS` to the authorized users' Microsoft sign-in addresses.
   No Azure app role assignment or new database table is required. Update the
   environment variable and restart/redeploy the server when changing access;
   the restart also clears existing in-memory sessions.
3. Create a client secret and configure `AZURE_TENANT_ID`, `AZURE_CLIENT_ID`,
   `AZURE_CLIENT_SECRET`, and `APP_ORIGIN` on the backend. Use the secret value,
   not its ID. Register `<APP_ORIGIN>/login` as an allowed Web redirect URI for
   the post-logout redirect as well. Implicit grant is not needed.

Copy `.env.example` to `.env`, fill in the values, then export them before
starting the server (the app does not automatically load dotenv files):

```sh
set -a
source .env
set +a
just run
```

Use `APP_ORIGIN=http://localhost:5173` while using Vite, or
`http://localhost:8080` when opening the built app directly. Redirects and origin
checks use this configured value, never incoming Host/forwarded headers. HTTP
is allowed only for loopback development. Staging/production require HTTPS.
Azure configuration is required in every environment; missing or invalid
values fail startup.

Sessions use an opaque HttpOnly, SameSite=Lax cookie, with Secure and the
`__Host-` prefix on HTTPS. Sessions expire at the earlier of the ID token's
expiration and one hour after login; allowlist changes require a server restart.
Login attempts expire after ten minutes, successful login replaces the session
ID, and logout deletes the server session before continuing to Azure sign-out.
No refresh tokens are stored. Sessions stay in process memory. A restart clears
them, and each process keeps its own sessions. Expired records are removed on
access and swept every minute, including abandoned sign-in attempts.

`GET /api/auth/session` returns the current user and CSRF token. Write requests
must send that token as `X-CSRF-Token`; browser Origin headers must match
`APP_ORIGIN`. `POST /api/auth/logout` uses the same check. Auth responses are
not cacheable. The frontend clears its query cache after logout and rechecks
sessions on focus, periodically, and after a 401/403 response.

Both React navigation and direct `/database` requests check admin access.
The backend protects all `/api` paths by default, including future member
handlers. Only the four exact auth endpoints, OpenAPI, and Swagger UI are public.
Register new member handlers under the existing API router to inherit this
protection; add their explicit response enums and OpenAPI documentation as usual.

Local tests cover token validation, session expiry/revocation, callback and
CSRF checks, and frontend access states. HTTP integration tests exercise the
application router with session cookies, including anonymous/non-admin denial,
admin access, write-request CSRF checks, expired sessions, and replaying a cookie
after logout. Token-validation tests generate temporary RSA signing keys in
memory; no private-key fixture is stored in the repository or written to disk.
A real Azure sign-in/logout check needs the configured app registration, an
assigned admin account, and a non-admin account.

Protocol reference: [Microsoft's OpenID Connect sign-in and sign-out documentation](https://learn.microsoft.com/en-us/entra/identity-platform/v2-protocols-oidc).

References: [Microsoft authorization-code flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow),
[Microsoft ID token claims](https://learn.microsoft.com/en-us/entra/identity-platform/id-token-claims-reference).

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
The schema includes `/version`, auth endpoints, session/user bodies, and error bodies.

The generated SDK and TanStack Query helpers live in that folder. Same-origin
`baseUrl` is set in `frontend/src/lib/api/hey-api.ts`. Put service hooks under
`lib/api/queries`. Auth queries and logout use the generated SDK.

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
| `AZURE_TENANT_ID` | required | Single-tenant Azure directory UUID |
| `AZURE_CLIENT_ID` | required | Azure Web application client UUID |
| `AZURE_CLIENT_SECRET` | required | Backend-only client secret |
| `AZURE_ADMIN_EMAILS` | Empty (deny all) | Comma-separated Microsoft sign-in emails allowed admin access |
| `APP_ORIGIN` | required | Public application origin, including port in development |
| `ENABLE_DOCUMENTATION` | `true` | Serve Swagger UI and OpenAPI routes |
| `HTTP_PORT` | `8080` | HTTP port; binds `0.0.0.0` |
| `VERSION` | unset | `/version` returns this, or `N/A` |
| `ENVIRONMENT` | unset | `production`/`staging` enables JSON logs |
| `RUST_LOG` | `info` | Tracing filter |

Health endpoints `/livez` and `/readyz` currently report process availability,
not upstream gRPC availability. `/metrics` exposes process metrics. The server
handles SIGINT/SIGTERM for graceful shutdown. Docker builds the frontend before
embedding it in the release binary and runs as a non-root user.
