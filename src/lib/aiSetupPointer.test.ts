import { describe, expect, it, vi } from "vitest";
import {
  AI_SETUP_FOCUS_KEY,
  AI_SETUP_GROUPS_KEY,
  AI_SETUP_PROVIDER_ID,
  goAiSetup,
  honorAiSetupFocus,
} from "./aiSetupPointer";

function memoryStores() {
  const sessionData: Record<string, string> = {};
  const localData: Record<string, string> = {};
  const session = {
    setItem: vi.fn((key: string, value: string) => {
      sessionData[key] = value;
    }),
    getItem: vi.fn((key: string) => sessionData[key] ?? null),
    removeItem: vi.fn((key: string) => {
      delete sessionData[key];
    }),
  };
  const local = {
    getItem: vi.fn((key: string) => localData[key] ?? null),
    setItem: vi.fn((key: string, value: string) => {
      localData[key] = value;
    }),
  };
  return { session, local, sessionData, localData };
}

describe("Quick Action setup pointer handoff (ticket 192)", () => {
  it("routes the plain Settings path with the AI-group flag and provider focus", async () => {
    const { session, local, sessionData, localData } = memoryStores();
    const goto = vi.fn();
    await goAiSetup({ goto, session, local });
    expect(goto).toHaveBeenCalledExactlyOnceWith("/settings");
    expect(sessionData[AI_SETUP_FOCUS_KEY]).toBe(AI_SETUP_PROVIDER_ID);
    expect(JSON.parse(localData[AI_SETUP_GROUPS_KEY])).toMatchObject({ ai: true });
  });

  it("leaves the other remembered groups alone when flagging the AI group", async () => {
    const { session, local, localData } = memoryStores();
    localData[AI_SETUP_GROUPS_KEY] = JSON.stringify({ general: false, ai: false });
    await goAiSetup({ goto: vi.fn(), session, local });
    expect(JSON.parse(localData[AI_SETUP_GROUPS_KEY])).toEqual({ general: false, ai: true });
  });

  it("still routes when storage is unavailable", async () => {
    const goto = vi.fn();
    const throwing = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
      removeItem: () => {
        throw new Error("denied");
      },
    };
    await goAiSetup({ goto, session: throwing, local: throwing });
    expect(goto).toHaveBeenCalledExactlyOnceWith("/settings");
  });

  it("honors the flag by expanding the AI group and focusing the provider, then consumes it", async () => {
    const { session, sessionData } = memoryStores();
    sessionData[AI_SETUP_FOCUS_KEY] = AI_SETUP_PROVIDER_ID;
    const expandAi = vi.fn();
    const focusProvider = vi.fn();
    const honored = await honorAiSetupFocus({
      session,
      expandAi,
      focusProvider,
      nextTick: () => Promise.resolve(),
    });
    expect(honored).toBe(true);
    expect(expandAi).toHaveBeenCalledTimes(1);
    expect(focusProvider).toHaveBeenCalledTimes(1);
    expect(sessionData[AI_SETUP_FOCUS_KEY]).toBeUndefined();
  });

  it("stays quiet without a flag and never touches focus", async () => {
    const { session } = memoryStores();
    const expandAi = vi.fn();
    const focusProvider = vi.fn();
    const honored = await honorAiSetupFocus({
      session,
      expandAi,
      focusProvider,
      nextTick: () => Promise.resolve(),
    });
    expect(honored).toBe(false);
    expect(expandAi).not.toHaveBeenCalled();
    expect(focusProvider).not.toHaveBeenCalled();
  });

  it("stays quiet when storage is unavailable", async () => {
    const throwing = {
      getItem: () => {
        throw new Error("denied");
      },
      removeItem: () => {
        throw new Error("denied");
      },
    };
    const honored = await honorAiSetupFocus({
      session: throwing,
      expandAi: vi.fn(),
      focusProvider: vi.fn(),
      nextTick: () => Promise.resolve(),
    });
    expect(honored).toBe(false);
  });
});
