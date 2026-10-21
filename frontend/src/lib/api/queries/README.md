Put service-specific React Query hooks here. Components consume hooks.
They should not define `useQuery` or `useMutation` inline.

Call the backend with the generated SDK and query helpers in
`@/lib/api/generated`. Do not add a custom request helper.

With the Rust backend running on port 8080, regenerate using
`pnpm --dir frontend generate`. Never edit `src/lib/api/generated` manually.
