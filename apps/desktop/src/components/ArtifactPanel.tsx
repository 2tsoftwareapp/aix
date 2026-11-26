import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Markdown from "react-markdown";
import type { StepId } from "../types";

const ARTIFACT_FOR_STEP: Partial<Record<StepId, string>> = {
  constitution: ".specify/memory/constitution.md",
  specify: "spec.md",
  clarify: "spec.md",
  plan: "plan.md",
  tasks: "tasks.md",
  checklist: "checklists/requirements.md",
  analyze: "tasks.md",
  implement: "tasks.md",
  converge: "tasks.md",
};

const TABS = [
  { rel: "spec.md", label: "spec" },
  { rel: "plan.md", label: "plan" },
  { rel: "tasks.md", label: "tasks" },
];

export function ArtifactPanel({
  step,
  projectRoot,
  featureSlug,
  refreshToken,
}: {
  step: StepId;
  projectRoot: string | null;
  featureSlug: string | null;
  refreshToken: number;
}) {
  const [rel, setRel] = useState<string | null>(ARTIFACT_FOR_STEP[step] ?? "spec.md");
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editing, setEditing] = useState(false);

  useEffect(() => {
    const forStep = ARTIFACT_FOR_STEP[step];
    if (forStep) setRel(forStep);
  }, [step]);

  const load = useCallback(async () => {
    if (!projectRoot || !rel) return;
    setError(null);
    try {
      const text = await invoke<string>("read_artifact", {
        projectRoot,
        featureSlug: rel.startsWith(".specify") ? null : featureSlug,
        relPath: rel,
      });
      setContent(text);
      setDirty(false);
    } catch (e) {
      setContent(null);
      setError(String(e));
    }
  }, [projectRoot, featureSlug, rel]);

  useEffect(() => {
    load();
  }, [load, refreshToken]);

  const save = async () => {
    if (!projectRoot || !rel || content === null) return;
    setSaving(true);
    try {
      await invoke("write_artifact", {
        projectRoot,
        featureSlug: rel.startsWith(".specify") ? null : featureSlug,
        relPath: rel,
        content,
      });
      setDirty(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <aside className="flex w-[28rem] flex-col border-l border-edge bg-surface-raised">
      <header className="flex items-center gap-1 border-b border-edge px-3 py-2">
        {TABS.map((t) => (
          <button
            key={t.rel}
            onClick={() => setRel(t.rel)}
            className={`rounded px-2 py-1 text-xs font-mono ${
              rel === t.rel ? "bg-surface-overlay text-ink" : "text-ink-faint hover:text-ink"
            }`}
          >
            {t.label}
          </button>
        ))}
        <span className="flex-1" />
        {featureSlug && (
          <span className="truncate max-w-32 font-mono text-[10px] text-ink-faint">{featureSlug}</span>
        )}
        {content !== null && !rel?.includes("checklists/") && (
          <button
            onClick={() => setEditing((v) => !v)}
            className="rounded border border-edge px-2 py-1 text-xs text-ink-dim hover:text-ink"
          >
            {editing ? "Preview" : "Edit"}
          </button>
        )}
        {dirty && editing && (
          <button
            onClick={save}
            disabled={saving}
            className="rounded bg-accent px-2 py-1 text-xs font-medium text-white hover:bg-accent-hover disabled:opacity-40"
          >
            {saving ? "Saving…" : "Save"}
          </button>
        )}
      </header>
      <div className="flex-1 overflow-hidden flex flex-col">
        {!projectRoot ? (
          <p className="px-4 py-4 text-sm text-ink-faint">No project open.</p>
        ) : content !== null && rel?.includes("checklists/") ? (
          <div className="flex-1 overflow-y-auto px-4 py-3">
            <p className="text-[10px] text-ink-faint mb-2 uppercase tracking-wide">
              Reviewer checklist — only you can flip these
            </p>
            {content.split("\n").map((line, i) => {
              const m = line.match(/^(\s*[-*]\s*)\[( |x|X)\]\s*(.*)$/);
              if (!m) {
                return line.trim() ? (
                  <p key={i} className="text-xs text-ink-faint leading-relaxed whitespace-pre-wrap">{line}</p>
                ) : (
                  <div key={i} className="h-2" />
                );
              }
              const checked = m[2].toLowerCase() === "x";
              return (
                <label key={i} className="flex items-start gap-2 py-1 cursor-pointer group">
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={async (e) => {
                      try {
                        await invoke("flip_checklist_item", {
                          projectRoot,
                          featureSlug,
                          relPath: rel,
                          lineIndex: i,
                          checked: e.target.checked,
                        });
                        await load();
                      } catch (err) {
                        setError(String(err));
                      }
                    }}
                    className="mt-0.5 accent-[#d97757]"
                  />
                  <span className={`text-xs leading-relaxed ${checked ? "text-ink-faint line-through" : "text-ink-dim"}`}>
                    {m[3]}
                  </span>
                </label>
              );
            })}
            <div className="mt-3 flex justify-end">
              <button
                onClick={() => setRel("spec.md")}
                className="text-[10px] text-ink-faint hover:text-ink"
              >
                ← back to artifacts
              </button>
            </div>
          </div>
        ) : content !== null && editing ? (
          <textarea
            value={content}
            onChange={(e) => {
              setContent(e.target.value);
              setDirty(true);
            }}
            spellCheck={false}
            className="flex-1 resize-none bg-transparent px-4 py-3 font-mono text-xs leading-relaxed text-ink-dim outline-none"
          />
        ) : content !== null ? (
          <div className="flex-1 overflow-y-auto px-4 py-3 artifact-markdown">
            <Markdown>{content}</Markdown>
          </div>
        ) : (
          <div className="px-4 py-4 text-xs text-ink-faint">
            {error?.includes("no such file") || error?.includes("No such file") || error?.includes("no feature") ? (
              <p>
                <code className="font-mono">{rel}</code> doesn't exist yet — it will appear after the{" "}
                {rel === "spec.md" ? "specify" : rel === "plan.md" ? "plan" : "tasks"} step runs.
              </p>
            ) : (
              <p className="text-red-400/80 break-all">{error ?? "loading…"}</p>
            )}
          </div>
        )}
      </div>
    </aside>
  );
}
