import { invoke } from "@/adapters";
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

vi.mock("@/adapters", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

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
    invokeMock.mockResolvedValue({
      connect: { authUrl: "https://r.example", authPublishableKey: "rk" },
    });

    const first = await loadConnectRuntimeConfig();
    const second = await loadConnectRuntimeConfig();
    expect(first).toEqual({ authUrl: "https://r.example", authPublishableKey: "rk" });
    expect(second).toBe(first);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("get_client_config");
    expect(resolveConnectEnabled(false)).toBe(true);
  });

  it("resolves null when the command is missing or malformed", async () => {
    invokeMock.mockRejectedValue(new Error("unknown command"));
    expect(await loadConnectRuntimeConfig()).toBeNull();

    resetConnectRuntimeConfigForTests();
    invokeMock.mockResolvedValue({ nope: 1 });
    expect(await loadConnectRuntimeConfig()).toBeNull();
  });
});
