# Companion site picker limit

## Question

How many Companion sites should the dock picker show before it stops being a fast palette, and what happens beyond that — in-dock search, lazy load, or a pinned subset managed in the main app?

Standing rules applied: research 0004 rules 1–3 (Priority+ overflow never justified with fewer than ~5; split by frequency; at most two disclosure levels — fast access here, configuration elsewhere), research 0006 patterns 1/4/11 (visibility on-surface, configuration elsewhere; controls near their object; content-gated Favorites model with no master switch), ADR-0022 (single isolated site, docked only; picker is a label-plus-chevron over saved sites).

## Sources

- Jakob Nielsen, *Progressive Disclosure*, NN/g (2006) — https://www.nngroup.com/articles/progressive-disclosure/ — rare features wait behind an obvious affordance (0004:2).
- Brad Frost / Michael Scharnagl, *Priority+ Navigation Pattern* via CSS-Tricks — https://css-tricks.com/the-priority-navigation-pattern/ — overflow ("more") menu exists for many items; cited in 0004:1 as never justified with fewer than ~5.
- Smashing Magazine, *Responsive Navigation Patterns* (citing NN/g's "if you can show navigation, show it") — https://www.smashingmagazine.com/2017/04/overview-responsive-navigation-patterns/ (0004:1).
- WAI-APG *Menu Button Pattern* — https://www.w3.org/WAI/ARIA/apg/patterns/menu-button/ — button-plus-chevron signifier, Enter/Space operation, expanded-state semantics (0004 evidence update 2026-09-08).
- Apple HIG, *Popovers* — https://developer.apple.com/design/human-interface-guidelines/popovers — popovers are transient, focused, dismissed on outside click; not a home for search-over-dozens.
- Microsoft, *Guidelines for app settings* — https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings — durable preferences belong on the dedicated Settings surface (0012:35-36).
- George Miller, *The Magical Number Seven, Plus or Minus Two* (1956) + Cowan (2001) 4±1 working-memory revision — short picker lists are scanned, long ones are searched; the crossover sits at ~5–7 items.
- Hick's law (Hick–Hyman) — selection time grows with log2(n+1); capping the dock set bounds one-click time.
- Notion Help Center, *Navigate with the sidebar* — https://www.notion.com/help/navigate-with-the-sidebar — Favorites appears once the first page is favorited, no master switch, per-item removal (0006:11). Precedent for pin-model over search-model at small scale.

## Decision

**Dock picker shows at most 5 pinned sites in user order, plus a `Manage in Sprout…` row. No search bar, no lazy load inside the dock. Everything else lives in the main-app Companion manager, which stays unlimited and owns search once the list grows (≈15+).**

- **N = 5.** 0004:1 sets the floor: overflow UI is never justified below ~5, so 5 is the largest list that never needs an overflow affordance. The dock is 340 physical px (`constants/window.rs`, 0004 constraint) — 5 short site names fit without truncation at real DPI; 6+ forces full→short→icon degradation (0004:4) in a surface that must stay one-click. Miller/Cowan put unaided scanning at ~4–7; 5 sits inside both. Hick's law keeps the worst-case pick bounded.
- **No dock search.** Search inside a 340 px auto-hide strip violates 0004:3 (never put settings in a quick-access surface) and duplicates the manager. Apple/Microsoft guidance both point durable list management at the full surface, not the transient one. The picker is a WAI-APG menu-button (name-plus-chevron, ADR-0022 amendment 2026-09-08), not a combo-box.
- **Shape.** Pinned-5 in user order + `Manage in Sprout…` row (0006:1 — visibility on-surface, configuration elsewhere). Pin/reorder/add/edit live in the main app next to their objects (0006:4). Unpinned sites are main-app-only until re-pinned — same grammar as Notion Favorites (0006:11) and the Quick Clips tab gating (0004 case study: surface absent until content, main app is the discoverability home).
- **Main app at 15+.** If saved sites reach ~15, the main-app manager (not the dock) gains a filter box + virtualized/lazy list. 15 is where scanning breaks down and filtering pays for itself; it never migrates into the dock.

## Rejected

- **Dock search bar / lazy-load-in-dock.** Turns the glanceable palette into a browser address bar; breaks 0004:3, fights the auto-hide reveal that already owns dock hover (0004 case-study precedent on hover-reveal rejection), and can't be operated at 340 px without truncation.
- **Showing all N sites in the dock (no cap).** Long dropdown in a thin strip; Hick's-law cost on every open; label truncation at real DPI.
- **Hard cap of 5 total sites.** Confuses the picker surface with the data model. Users may save dozens; only the dock's visible set is capped.

## Consequences

- `companionUrlList` stays unbounded in settings/backup; dock reads first-5-pinned (user order) + manage row. Backup format unchanged.
- Picker marks the active choice; choosing a site follows the accepted single-live-page lifecycle (ADR-0022 amendments 2026-09-08/09); choosing off-list content is impossible by construction — the user re-pins in the main app.
- Main-app filter/virtualization is deferred until real lists approach 15; no dock work then.

## Evidence status

Synthesis from cited standing rules + platform guidance; no new user study. Revisit N only with dock-width measurements at 150%/200% DPI or picker timing data showing 5 still truncates.
