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
