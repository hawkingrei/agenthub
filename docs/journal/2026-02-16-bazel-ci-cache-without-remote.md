# Bazel CI Cache Setup Without Remote Cache

## Summary

Enable `setup-bazel` local/GitHub cache layers for the Bazel workflow while
keeping remote cache integration disabled.

## Background

The project currently runs a dedicated Bazel CI workflow but does not have
remote cache credentials (`google-credentials`) or extra secret bazelrc inputs.

Without explicit cache settings, CI repeatedly downloads Bazelisk artifacts and
external repositories, and re-executes many build actions.

## Scope

- `.github/workflows/bazel.yml`
- `docs/todo.md`

## Key Decisions

1. Upgrade to `bazel-contrib/setup-bazel@0.18.0`.
2. Enable cache layers that do not require remote credentials:
   - `bazelisk-cache: true`
   - `repository-cache: true`
   - `disk-cache: ${{ github.workflow }}`
3. Explicitly disable external repository cache (`external-cache: false`) for
   now to keep behavior predictable while remote cache is not configured.
4. Do not configure `google-credentials` or `bazelrc` secrets in this stage.

## Validation

```bash
bazel build //...
bazel test --test_output=errors //...
```

Expected:

- Bazel workflow remains functionally identical.
- Subsequent CI runs restore caches and reduce warm-up/rebuild time.

## Follow-ups

- Evaluate enabling `external-cache: true` after observing cache size and hit ratio.
- Add remote cache credentials and secure bazelrc once infrastructure is ready.
