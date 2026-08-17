# Summary

The Dependabot `jsdom 29.1.1 -> 30.0.1` bump (PR #983) failed CI's `Web` job with
`TypeError: webidl.util.markAsUncloneable is not a function`, thrown from `undici`'s `CacheStorage`
constructor during jsdom's module load -- so every test file that imports jsdom failed to even start.
This reproduced on CI (Node 20) but not locally (Node 26), which pointed at a Node-version
incompatibility rather than an application bug.

# Background

`jsdom@30.0.1` depends on `undici@8.10.0`, whose `lib/web/webidl/index.js` unconditionally imports
`markAsUncloneable` from Node's built-in `node:worker_threads` module. That export was added in Node
22; on Node 20 it's simply absent, so the import silently resolves to `undefined` and the later call
throws "is not a function" the moment any jsdom-backed test constructs a `CacheStorage`. `jsdom@30`'s
own `package.json` confirms this isn't an accident: `engines.node` is
`"^22.22.2 || ^24.15.0 || >=26.0.0"` -- Node 20 is explicitly unsupported. The `Web`, `Web E2E`, and
`Web E2E Mobile` GitHub Actions jobs (all of which install `web/package-lock.json`) were still pinned
to `node-version: "20"`.

# Scope

- `.github/workflows/web.yml`, `.github/workflows/e2e.yml`, `.github/workflows/e2e-mobile.yml`: bumped
  `node-version` from `"20"` to `"22"`.
- Left `.github/workflows/userdocs.yml` (also pinned to Node 20) untouched -- it builds the Docusaurus
  site from `userdocs/package-lock.json`, an entirely separate dependency graph with no jsdom
  dependency, so it isn't affected by this bump and bumping it would be an unrelated change.
- Left `release.yml`'s existing `node-version: 22` step alone -- already ahead of this baseline.

# Key Decisions

- Bumped CI to Node 22 rather than holding/closing the jsdom Dependabot PR, confirmed with the person
  requesting this fix: jsdom 30 is a real, low-risk correctness/security update, and Node 20's LTS
  window is closing, so raising the Web-job Node floor is the more durable fix versus staying pinned to
  a version the tooling itself no longer supports.
- Did not touch `userdocs.yml` -- scoping the Node bump to the jobs that actually install
  `web/package-lock.json` avoids an unrelated, unrequested change to a workflow this PR doesn't affect.
- No `web/package.json` `engines` field exists to keep in sync, and no `.nvmrc`/`vercel.json` pins a
  Node version elsewhere in the repo, so this change is CI-only.

# Validation

- `cd web && npm ci --legacy-peer-deps && npm run lint && npm exec tsc -- --noEmit` -- clean.
- `cd web && npm run test -- --run` -- 1524 passed (162 files), 0 failed -- confirms jsdom 30 itself
  works correctly once run on a Node version it actually supports.
- `cd web && npm run build` -- succeeds.
- CI verification (the actual point of this change) happens on the PR itself, since the failure only
  reproduces under CI's pinned Node version, not locally.

# Follow-Ups

- None. `userdocs.yml` staying on Node 20 is a deliberate, separate scope, not a gap.
