---
sidebar_position: 3
---

# Deploy User Docs on Vercel (Static)

This page is only for publishing the Docusaurus user documentation site.
It does not deploy the AgentHub backend service.

## Goal

Use `userdocs/` as a pure static site generator and host the output on Vercel.

## Local Parity Check

Before pushing, verify local static build:

```bash
cd userdocs
npm install
npm run build
```

Build output should appear in `userdocs/build/`.

## Vercel Project Settings

Use these values in Vercel:

- Root Directory: `userdocs`
- Install Command: `npm install`
- Build Command: `npm run build`
- Output Directory: `build`

## Branch Workflow Recommendation

- Use preview deployments for feature branches
- Use production deployment from your main branch
- Review sidebar links and major pages in preview before promote

## Common Failure Cases

- Wrong root directory (not `userdocs`)
- Wrong output directory (must be `build`)
- Missing dependencies due to skipped `npm install`

## Validation After Deploy

1. Open the deployed URL.
2. Navigate every sidebar category once.
3. Confirm no broken links or 404 pages.
4. Confirm deployment page URLs are stable after next build.

## Related Pages

- [AgentHub User Guide Intro](../intro.md)
- [Troubleshooting](../operations/troubleshooting.md)
