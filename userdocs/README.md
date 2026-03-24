# AgentHub User Docs

This directory contains the published end-user documentation site for
AgentHub.

Published site:

- `https://doc.agenthub.hawkingrei.com/`

## Local Preview

```bash
npm --prefix userdocs ci
npm --prefix userdocs run start
```

Default preview URL is `http://localhost:3000`.

## Build Static Site

```bash
npm --prefix userdocs run build
```

Static output is generated at `userdocs/build/`.

## Serve Built Output Locally

```bash
npm --prefix userdocs run serve
```

## Deploy Notes

The `userdocs/` site is built as a static Docusaurus site and is suitable for
static hosting.

Recommended static-host settings:

- Root directory: `userdocs`
- Install command: `npm ci`
- Build command: `npm run build`
- Output directory: `build`

## Content Organization

- `docs/`: user-facing pages
- `sidebars.js`: navigation structure
- `docusaurus.config.js`: site-level metadata and routing
- `src/css/custom.css`: docs-site styling
