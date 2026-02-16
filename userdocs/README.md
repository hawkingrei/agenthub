# AgentHub User Docs (Docusaurus)

This directory contains the end-user documentation site for AgentHub.
It is used as a static site generator (no runtime backend required for hosting docs).

## Local Preview

```bash
cd userdocs
npm install
npm run start
```

Default preview URL is `http://localhost:3000`.

## Build Static Site

```bash
cd userdocs
npm run build
```

Static output is generated at `userdocs/build/`.

## Serve Built Output Locally

```bash
cd userdocs
npm run serve
```

## Deploy on Vercel (Static Hosting)

Recommended project settings:

- Root Directory: `userdocs`
- Install Command: `npm install`
- Build Command: `npm run build`
- Output Directory: `build`

## Content Organization

- `docs/`: user-facing pages
- `sidebars.js`: navigation structure
- `docusaurus.config.js`: site-level config
- `src/css/custom.css`: docs site styling
