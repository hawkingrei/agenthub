# Summary

`web/public/pwa-192.png` and `pwa-512.png` -- the favicon and PWA install icons -- were not an
AgentHub brand mark at all. They were regenerated in
[journal/2026-08-16-pwa-icon-branding-fix.md](2026-08-16-pwa-icon-branding-fix.md) from
`web/public/slock-icon.png`, which `git log --follow` shows was added in #644 ("add Slock OAuth
linkers") specifically to label the "Slock Linker" card in `admin_page_sections.tsx` -- a third-party
OAuth identity provider AgentHub integrates with, not AgentHub itself. That prior fix mistook the only
icon-shaped asset in the repo for the app's own logo and wired it into the browser tab, PWA home-screen
icon, and `apple-touch-icon`, so every installed/bookmarked instance of AgentHub displayed another
product's mark.

# Scope

- Regenerated `web/public/pwa-192.png` and `pwa-512.png` as a neutral placeholder: a rounded-square
  navy tile (`#0f172a`, matches `manifest.webmanifest`'s `theme_color`) with a bold "A" monogram in the
  app's existing Mantine `notion-accent` primary color (`#2383e2`, from `web/src/ui/mantine_theme.ts`).
  Rendered at 512px with Pillow/Arial Black, downsampled to 192px with Lanczos.
- `web/index.html`'s `<link rel="icon">`/`apple-touch-icon` and `manifest.webmanifest`'s icon entries
  already pointed at these filenames -- only the image bytes changed, no markup/manifest edits needed.
- Left `web/public/slock-icon.png` and its usage in `admin_page_sections.tsx` untouched: that is its
  correct, original, intended use (labeling the Slock integration card), not part of this bug.

# Key Decisions

- Placeholder, not a fabricated "final" logo: there is no AgentHub brand asset anywhere in the repo to
  fall back to. Confirmed with the person requesting this fix that a simple letter-mark placeholder
  (clearly generated, not claiming to be a designed brand identity) is preferred over leaving the
  borrowed Slock mark in place while a real design is pending, and over blocking on a real logo being
  supplied first.
- Kept the same 192/512 filenames and manifest contract (no `purpose: any maskable` declaration, same
  as the prior fix) so this is a pure asset swap, not a contract change.

# Validation

- `cd web && npm exec vitest -- run src/pwa_public_assets.test.ts` -- 2 passed, manifest icon
  `src`/`sizes`/`type` contract unaffected.
- `cd web && npm run build` -- succeeds; `dist/pwa-192.png`/`dist/pwa-512.png` byte-match the new
  `public/` assets (verified by hash).
- `cd web && npm run test -- --run` -- 1521 of 1522 passed; the one failure
  (`team_page.agent_loop.test.tsx`) is a pre-existing test-isolation flake unrelated to this change --
  confirmed by rerunning that file alone, which passes cleanly.

# Follow-Ups

- This placeholder still needs replacing with a real, designed AgentHub brand mark. When one exists,
  regenerate `pwa-192.png`/`pwa-512.png` from a native high-resolution export (per the still-open
  low-source-resolution risk noted in `features/web-static-assets-and-pwa.md`), not by re-deriving from
  this placeholder.
