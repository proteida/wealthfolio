import {
  isRuntimeConnectConfigured,
  loadConnectRuntimeConfig,
  resetConnectRuntimeConfigForTests,
  resolveAuthPublishableKey,
  resolveAuthUrl,
  resolveConnectEnabled,
  resolveOAuthCallbackUrl,
} from "@/lib/connect-runtime-config";
import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(() => {
  resetConnectRuntimeConfigForTests();
  vi.restoreAllMocks();
});

describe("connect runtime config", () => {
  it("falls back to baked values, then cloud defaults", () => {
    expect(resolveAuthUrl(null, {})).toBe("https://auth.wealthfolio.app");
    expect(
      resolveAuthUrl(null, { authUrl: "https://baked.example" }),
    ).toBe("https://baked.example");
    expect(
      resolveAuthUrl({ authUrl: "https://runtime.example" }, { authUrl: "https://baked.example" }),
    ).toBe("https://runtime.example");
    expect(resolveAuthPublishableKey(null, {})).toContain("sb_publishable_");
    expect(resolveOAuthCallbackUrl(null, {})).toBe("https://connect.wealthfolio.app/deeplink");
  });

  it("counts as configured only with auth URL + key", () => {
    expect(isRuntimeConnectConfigured(null)).toBe(false);
    expect(isRuntimeConnectConfigured({ authUrl: "https://a.example" })).toBe(false);
    expect(
      isRuntimeConnectConfigured({ authUrl: "https://a.example", authPublishableKey: "k" }),
    ).toBe(true);
  });

  it("enables on baked config or runtime override", () => {
    expect(resolveConnectEnabled(false)).toBe(false);
    expect(resolveConnectEnabled(true)).toBe(true);
  });

  it("loads once from /api/v1/client-config and tolerates failure", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: () =>
        Promise.resolve({
          connect: { authUrl: "https://r.example", authPublishableKey: "rk" },
        }),
    });
    vi.stubGlobal("fetch", fetchMock);

    const first = await loadConnectRuntimeConfig();
    const second = await loadConnectRuntimeConfig();
    expect(first).toEqual({ authUrl: "https://r.example", authPublishableKey: "rk" });
    expect(second).toBe(first);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledWith("/api/v1/client-config", expect.anything());
    expect(resolveConnectEnabled(false)).toBe(true);
  });

  it("resolves null when the endpoint is missing or malformed", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: false, json: () => Promise.resolve({}) }),
    );
    expect(await loadConnectRuntimeConfig()).toBeNull();

    resetConnectRuntimeConfigForTests();
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, json: () => Promise.resolve({ nope: 1 }) }),
    );
    expect(await loadConnectRuntimeConfig()).toBeNull();
  });
});
