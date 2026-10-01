/**
 * Runtime Connect endpoints, served by the backend.
 *
 * `GET /api/v1/client-config` reports the `WF_CONNECT_*` overrides from
 * `wealthfolio.env`, so self-hosters can repoint the connector/auth host with
 * an edit + restart instead of a rebuild. Everything here is optional:
 * absent (fetch fails, endpoint missing, or a key unset) means "no runtime
 * override" and every resolver below falls back to its baked-in build value.
 *
 * Only publishable material flows through this channel — never secrets.
 */

import { invoke } from "@/adapters";

export interface RuntimeConnectConfig {
  apiUrl?: string;
  authUrl?: string;
  authPublishableKey?: string;
  oauthCallbackUrl?: string;
}

let slot: RuntimeConnectConfig | null = null;
let inflight: Promise<RuntimeConnectConfig | null> | null = null;
let settled = false;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function clean(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

/** Fetch once (cached); resolves `null` on any failure. Safe pre-auth. */
export function loadConnectRuntimeConfig(): Promise<RuntimeConnectConfig | null> {
  if (settled) return Promise.resolve(slot);
  if (!inflight) {
    // Adapter-routed: Tauri IPC on desktop, GET /api/v1/client-config on web.
    inflight = (async () => {
      try {
        const body = await invoke<unknown>("get_client_config");
        if (!isRecord(body) || !isRecord(body.connect)) return null;
        const c = body.connect;
        const cfg: RuntimeConnectConfig = {
          apiUrl: clean(c.apiUrl),
          authUrl: clean(c.authUrl),
          authPublishableKey: clean(c.authPublishableKey),
          oauthCallbackUrl: clean(c.oauthCallbackUrl),
        };
        return cfg.apiUrl ?? cfg.authUrl ?? cfg.authPublishableKey ?? cfg.oauthCallbackUrl
          ? cfg
          : null;
      } catch {
        return null;
      }
    })().then((cfg) => {
      slot = cfg;
      settled = true;
      return cfg;
    });
  }
  return inflight;
}

/** Synchronous read of the last loaded config (`null` until loaded). */
export function getRuntimeConnectConfig(): RuntimeConnectConfig | null {
  return slot;
}

/** Test seam: reset the cache. */
export function resetConnectRuntimeConfigForTests(): void {
  slot = null;
  inflight = null;
  settled = false;
}

/** Runtime pair counts as configured when it carries auth URL + key. */
export function isRuntimeConnectConfigured(
  cfg: RuntimeConnectConfig | null = slot,
): boolean {
  return Boolean(cfg?.authUrl && cfg?.authPublishableKey);
}

/** Effective flag: baked build config OR a runtime override. */
export function resolveConnectEnabled(bakedEnabled: boolean): boolean {
  return bakedEnabled || isRuntimeConnectConfigured();
}

function pick(runtime: string | undefined, baked: string | undefined, fallback: string): string {
  return runtime ?? baked ?? fallback;
}

/** Test seam for the baked build values (import.meta.env in production). */
export interface BakedConnectValues {
  authUrl?: string;
  authPublishableKey?: string;
  oauthCallbackUrl?: string;
}

export function resolveAuthUrl(
  cfg: RuntimeConnectConfig | null = slot,
  baked?: BakedConnectValues,
): string {
  return pick(cfg?.authUrl, baked?.authUrl, "https://auth.wealthfolio.app");
}

export function resolveAuthPublishableKey(
  cfg: RuntimeConnectConfig | null = slot,
  baked?: BakedConnectValues,
): string {
  return pick(
    cfg?.authPublishableKey,
    baked?.authPublishableKey,
    "sb_publishable_ZSZbXNtWtnh9i2nqJ2UL4A_NV8ZVutd",
  );
}

export function resolveOAuthCallbackUrl(
  cfg: RuntimeConnectConfig | null = slot,
  baked?: BakedConnectValues,
): string {
  return pick(
    cfg?.oauthCallbackUrl,
    baked?.oauthCallbackUrl,
    "https://connect.wealthfolio.app/deeplink",
  );
}
