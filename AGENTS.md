# Campus Pilot Project Rules

These rules apply to the whole repository. Backend-specific conventions remain in `apis/AGENTS.md`.

## Canonical references

- Read `docs/design-system.md` before changing the client UI.
- Read `docs/access-control.md` before changing sign-in, roles, permissions, module navigation, or licensing.
- Keep that file current when a new durable UI rule or interaction decision is agreed.
- Do not create another rules document unless it covers a genuinely separate concern.

## Product and UI

- Preserve Campus Pilot behavior and school-domain language while adapting the structural quality of `/Users/modestnerd/Developer/Projects/ccs`.
- Do not use centered modals. Forms, confirmations, and secondary workflows use accessible right-side drawers.
- The admin sidebar must remain scrollable without showing native scrollbar chrome.
- Use the shared tokens and primitives in `client/src/styles/tokens.css` and `client/src/components/ui/`; avoid literal color systems and one-off component chrome.
- Never present invented operational data as real. Prefer honest empty, loading, error, or setup states.
- Route every navigation item to a real screen or an intentional coming-soon state; do not leave dead links.

## Change discipline

- Preserve unrelated local changes in the working tree.
- Verify UI changes with `pnpm run build` from `client/` and with browser checks at desktop and mobile widths.
- For image generation or editing, use the project-approved image generation skill only.

## Docker deployment on this host

Always include both Compose files so Traefik routing and the production API build argument are retained:

```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml build client
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d --no-deps client
```

For API changes, build and restart `apis` with both files as documented in `README.md`. Verify container health and the public route after deployment.
