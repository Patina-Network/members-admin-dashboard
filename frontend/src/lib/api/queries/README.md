Put service-specific React Query hooks here, following CodeBloom's
`lib/api/queries/<service>/index.ts` structure. Components consume hooks;
they should not define `useQuery` or `useMutation` inline.

Use the generated types in `@/lib/api/types/schema`. Add shared typed request
helpers in `lib/api/common` following CodeBloom's `ApiURL` pattern when adding
the first feature endpoints. No requests are wired into the static scaffold.

With the Rust backend running on port 8080, regenerate types using
`pnpm --dir frontend generate`. Never edit `schema.ts` manually.
