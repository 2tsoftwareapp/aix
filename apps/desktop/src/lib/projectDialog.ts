import { open } from "@tauri-apps/plugin-dialog";

export async function chooseProjectDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, title: "Open project" });
  return typeof selected === "string" ? selected : null;
}
