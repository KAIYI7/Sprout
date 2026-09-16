# 215 — Frontend Mac build flag (route exclusion)

**Parent:** [206 — Mac Quick-access port, portable Python shell, and dual-train releases (spec)](206-mac-quick-access-port-python-release-spec.md)

**What to build:** The Mac bundle contains no installer configuration surfaces: the installer routes are excluded at build time while every shared component, token, and type stays common — and the Windows bundle is byte-comparable in behavior to before.

**Blocked by:** 208 (needs the per-platform route list).

**Status:** ready-for-agent

- [ ] Build flag excludes the installer routes (products/presets/plan and their nav) from the Mac bundle; hidden-nav is not accepted as exclusion
- [ ] Shared layer (components, design tokens, API types) remains the single source for both bundles
- [ ] Bundle-size check recorded for both platforms (Mac establishes its baseline; Windows shows no unexplained growth)
- [ ] Verification: both frontend bundles build; checks green; exclusion asserted by route inventory, not screenshots

**Explicitly not built:**

- Any route redesign or new Mac-only screens
