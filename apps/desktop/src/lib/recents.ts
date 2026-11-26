const KEY = "aix.recentProjects";
const LAST_KEY = "aix.lastProject";

export function getRecents(): string[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "[]");
  } catch {
    return [];
  }
}

export function addRecent(path: string) {
  const list = [path, ...getRecents().filter((p) => p !== path)].slice(0, 6);
  localStorage.setItem(KEY, JSON.stringify(list));
  localStorage.setItem(LAST_KEY, path);
}

export function getLastProject(): string | null {
  return localStorage.getItem(LAST_KEY);
}
