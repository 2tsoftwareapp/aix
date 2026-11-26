export interface ModelSearchItem {
  id: string;
  label: string;
  auxiliaryData?: {
    custom?: boolean;
    vendor?: string;
    contextLength?: number;
  };
}

export interface CatalogEntry {
  id: string;
  name?: string;
  context_length?: number | null;
}

/** "anthropic/claude-sonnet-4.5" → "anthropic"; bare ids → "" */
export function vendorOf(id: string): string {
  const slash = id.indexOf("/");
  return slash > 0 ? id.slice(0, slash) : "";
}

export function formatContextLength(n?: number | null): string {
  if (!n) return "";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n % 1_000_000 === 0 ? 0 : 1)}M ctx`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}k ctx`;
  return `${n} ctx`;
}

/** Bare-id search for direct providers (anthropic/openai/qwen). */
export function searchModelItems(modelIds: string[], query: string): ModelSearchItem[] {
  const trimmed = query.trim();
  const needle = trimmed.toLowerCase();
  const matches = modelIds
    .filter((id) => !needle || id.toLowerCase().includes(needle))
    .map((id) => ({ id, label: id }));

  if (!trimmed) return matches;
  if (modelIds.includes(trimmed)) return matches;
  return [...matches, { id: trimmed, label: trimmed, auxiliaryData: { custom: true } }];
}

/**
 * Catalog-driven search for OpenRouter: matches display name AND id,
 * carries vendor + context length so the picker can render rich rows.
 */
export function searchCatalogItems(entries: CatalogEntry[], query: string): ModelSearchItem[] {
  const trimmed = query.trim();
  const needle = trimmed.toLowerCase();
  const matches = entries
    .filter((e) =>
      !needle
      || e.id.toLowerCase().includes(needle)
      || (e.name ?? "").toLowerCase().includes(needle))
    .map((e) => ({
      id: e.id,
      label: e.name || e.id,
      auxiliaryData: {
        vendor: vendorOf(e.id),
        contextLength: e.context_length ?? undefined,
      },
    }));

  if (!trimmed) return matches;
  if (entries.some((e) => e.id === trimmed)) return matches;
  return [...matches, { id: trimmed, label: trimmed, auxiliaryData: { custom: true } }];
}

/** Stable ordering: vendor asc, then display name asc. */
export function sortCatalog(entries: CatalogEntry[]): CatalogEntry[] {
  return [...entries].sort((a, b) => {
    const va = vendorOf(a.id);
    const vb = vendorOf(b.id);
    if (va !== vb) return va.localeCompare(vb);
    return (a.name || a.id).localeCompare(b.name || b.id);
  });
}

export function catalogStats(entries: CatalogEntry[]): { models: number; vendors: number } {
  return { models: entries.length, vendors: new Set(entries.map((e) => vendorOf(e.id)).filter(Boolean)).size };
}
