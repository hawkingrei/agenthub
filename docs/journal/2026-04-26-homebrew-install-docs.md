# Homebrew Install Docs

## Summary

- added a first-class Homebrew install path to the root `README.md`
- aligned `userdocs/docs/getting-started/installation.md` with the same tap and
  `brew services` flow
- clarified that the source-based `make run` path is the development workflow,
  not the primary release-binary install path

## Scope

- root `README.md`
- `userdocs/docs/getting-started/installation.md`

## Notes

- the published tap is `linkerdog/homebrew-tap`
- the recommended install command uses the fully qualified formula name:
  `brew install linkerdog/homebrew-tap/agenthub`
- current published binaries cover:
  - macOS Apple Silicon
  - Linux `x86_64`
  - Linux `aarch64`
