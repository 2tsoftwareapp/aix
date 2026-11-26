export interface ModelSearchItem {
  id: string;
  label: string;
  auxiliaryData?: { custom?: boolean };
}

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
