/// The Quick Action empty-state pointer's handoff to Settings (ticket 192).
 ///
 /// The unready dialog keeps zero AI chrome and offers at most one plain-text
 /// line into Settings. The handoff itself is plain navigation plus a session
 /// flag — no per-group `/settings#` route, no smooth-scroll library, no
 /// pulse. Settings honors the flag by expanding the AI group and focusing
 /// the provider control; the focus ring is the only highlight.
 ///
 /// Both sides share this module so the storage keys cannot drift between
 /// the writer (the dialog's page) and the reader (Settings).

/** Session flag asking Settings to expand the AI group and focus it. */
export const AI_SETUP_FOCUS_KEY = "sprout.settings.focus";

/** Settings' remembered group-open map (owned by the Settings page). */
export const AI_SETUP_GROUPS_KEY = "sprout.settings.groups.v1";

/** The provider control the pointer focuses after landing. */
export const AI_SETUP_PROVIDER_ID = "ai-provider";

export interface AiSetupWriter {
  goto: (path: string) => void | Promise<void>;
  session: Pick<Storage, "setItem">;
  local: Pick<Storage, "getItem" | "setItem">;
}

/** Routes the unready pointer to the Settings page (ticket 192): closes over
 *  the dialog first (the caller), then flags the AI group + provider focus
 *  and navigates the plain route. Storage failures degrade to a plain
 *  Settings visit — the page still opens on its own route. */
export async function goAiSetup(deps: AiSetupWriter): Promise<void> {
  try {
    deps.session.setItem(AI_SETUP_FOCUS_KEY, AI_SETUP_PROVIDER_ID);
  } catch {
    // Storage unavailable — Settings still opens on its own route.
  }
  try {
    const raw = deps.local.getItem(AI_SETUP_GROUPS_KEY);
    const parsed = raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
    deps.local.setItem(AI_SETUP_GROUPS_KEY, JSON.stringify({ ...parsed, ai: true }));
  } catch {
    // Storage unavailable — the AI group still opens on first visit.
  }
  await deps.goto("/settings");
}

export interface AiSetupReader {
  session: Pick<Storage, "getItem" | "removeItem">;
  expandAi: () => void;
  focusProvider: () => void;
  nextTick: () => Promise<void>;
}

/** Honors the pointer flag on Settings mount (ticket 192): expands the AI
 *  group and focuses the provider control with a plain focus call — no
 *  smooth scroll, no pulse. Returns true only when a flag was honored; the
 *  flag is always consumed so a later visit starts clean. */
export async function honorAiSetupFocus(deps: AiSetupReader): Promise<boolean> {
  let flagged = false;
  try {
    flagged = deps.session.getItem(AI_SETUP_FOCUS_KEY) === AI_SETUP_PROVIDER_ID;
    if (flagged) deps.session.removeItem(AI_SETUP_FOCUS_KEY);
  } catch {
    return false;
  }
  if (!flagged) return false;
  deps.expandAi();
  await deps.nextTick();
  deps.focusProvider();
  return true;
}
