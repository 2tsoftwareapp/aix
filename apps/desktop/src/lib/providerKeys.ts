// Shared provider-key status store. Module-level pub/sub so every consumer
// (chat header dots, composer enablement, settings dialog) updates from the
// same source the instant a key is stored or deleted — no prop threading.
import { invoke } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";

export interface VendorInfo {
  id: string;
  display: string;
  kind: string;
  env: string;
  placeholder: string;
  source: string;
  note: string | null;
}

/** @deprecated use useVendorConfigs / vendorIds — kept for migration */
export const PROVIDER_IDS = ["anthropic", "openai", "openrouter", "qwen"] as const;
export type ProviderId = (typeof PROVIDER_IDS)[number];

let status: Record<string, boolean> = {};
let vendors: VendorInfo[] = [];
let version = 0;
const listeners = new Set<() => void>();

function emit() {
  version++;
  listeners.forEach((l) => l());
}

export async function refreshKeyStatus(): Promise<Record<string, boolean>> {
  try {
    vendors = await invoke<VendorInfo[]>("list_vendor_configs");
  } catch {
    vendors = [];
  }
  const ids = vendors.length > 0 ? vendors.map((v) => v.id) : [...PROVIDER_IDS];
  // keep entries for any already-configured key even if the vendor list shrinks
  const allIds = [...new Set([...ids, ...Object.keys(status)])];
  const entries = await Promise.all(
    allIds.map(
      async (p) => [p, await invoke<boolean>("has_provider_key", { providerId: p }).catch(() => false)] as const
    )
  );
  status = Object.fromEntries(entries);
  emit();
  return status;
}

export function useVendorConfigs(): VendorInfo[] {
  useKeyStatus();
  return vendors;
}

export function vendorDisplay(id: string): string {
  return vendors.find((v) => v.id === id)?.display ?? id;
}

export async function storeKey(providerId: string, key: string): Promise<void> {
  await invoke("store_provider_key", { providerId, key });
  await refreshKeyStatus();
}

export async function deleteKey(providerId: string): Promise<void> {
  await invoke("delete_provider_key", { providerId });
  await refreshKeyStatus();
}

export function useKeyStatus(): Record<string, boolean> {
  useSyncExternalStore(
    (cb) => {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    () => version
  );
  return status;
}

// kick initial load once at module import
void refreshKeyStatus();
