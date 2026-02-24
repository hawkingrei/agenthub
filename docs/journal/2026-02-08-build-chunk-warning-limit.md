# Build Chunk Warning Limit

## Background

Vite emitted chunk size warnings after adding frontend dependencies, even though the build output is acceptable for now. The warnings can distract from real issues during CI.

## Scope

- Increase the Rollup chunk size warning threshold for the web build.

## Key Decisions

- Set `build.chunkSizeWarningLimit` to `1500` to quiet warnings for the current bundle size while keeping a reasonable ceiling.

## Validation

```bash
cd web
npm run build
```
