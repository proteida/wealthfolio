import { afterEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ profileFetch: vi.fn() }));
vi.mock("@/features/profiles/session", () => ({ profileFetch: mocks.profileFetch }));

import { invoke } from "./core";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetAllMocks();
});

it("reads public client config without a profile scope", async () => {
  const config = { connect: { apiUrl: "https://custom.example" } };
  const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify(config)));
  vi.stubGlobal("fetch", fetch);
  await expect(invoke("get_client_config")).resolves.toEqual(config);
  expect(fetch).toHaveBeenCalledWith(
    "/api/v1/client-config",
    expect.objectContaining({ method: "GET" }),
  );
  expect(mocks.profileFetch).not.toHaveBeenCalled();
});

it("keeps financial requests on the profile-scoped transport", async () => {
  const fetch = vi.fn();
  vi.stubGlobal("fetch", fetch);
  mocks.profileFetch.mockRejectedValue(new Error("PROFILE_LOCKED"));
  await expect(invoke("get_accounts")).rejects.toThrow("PROFILE_LOCKED");
  expect(fetch).not.toHaveBeenCalled();
});
