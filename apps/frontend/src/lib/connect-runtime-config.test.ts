import { getClientConfig } from "@/adapters";
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

vi.mock("@/adapters", () => ({ getClientConfig: vi.fn() }));

const clientConfigMock = vi.mocked(getClientConfig);

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

  it("loads once via the adapter and tolerates failure", async () => {
    clientConfigMock.mockResolvedValue({
      connect: { authUrl: "https://r.example", authPublishableKey: "rk" },
    });

    const first = await loadConnectRuntimeConfig();
    const second = await loadConnectRuntimeConfig();
    expect(first).toEqual({ authUrl: "https://r.example", authPublishableKey: "rk" });
    expect(second).toBe(first);
    expect(clientConfigMock).toHaveBeenCalledTimes(1);
    expect(resolveConnectEnabled(false)).toBe(true);
  });

  it("resolves null when the command is missing or malformed", async () => {
    clientConfigMock.mockRejectedValue(new Error("unknown command"));
    expect(await loadConnectRuntimeConfig()).toBeNull();

    resetConnectRuntimeConfigForTests();
    clientConfigMock.mockResolvedValue({ nope: 1 } as never);
    expect(await loadConnectRuntimeConfig()).toBeNull();
  });
});
