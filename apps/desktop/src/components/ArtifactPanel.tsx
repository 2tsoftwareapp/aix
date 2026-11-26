import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Markdown from "react-markdown";
import {
  Pencil, Eye, Copy, Check, FileText, ClipboardCheck, FolderOpen, Loader2,
} from "lucide-react";
import { Button } from "./ui/button";
import { Badge } from "./ui/badge";
import { Checkbox } from "./ui/checkbox";
import { Progress } from "./ui/progress";
import { ScrollArea } from "./ui/scroll-area";
import { toast } from "sonner";
import { cn } from "../lib/utils";
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

function EmptyState({ icon: Icon, title, body }: { icon: typeof FileText; title: string; body: string }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
      <div className="flex h-10 w-10 items-center justify-center rounded-full bg-secondary">
        <Icon className="h-4 w-4 text-muted-foreground" />
      </div>
      <p className="text-xs font-medium">{title}</p>
      <p className="text-[11px] leading-relaxed text-muted-foreground">{body}</p>
    </div>
  );
}

export function ArtifactPanel({
  step,
  projectRoot,
  featureSlug,
  refreshToken,
  checklists,
}: {
  step: StepId;
  projectRoot: string | null;
  featureSlug: string | null;
  refreshToken: number;
  checklists: string[];
}) {
  const [rel, setRel] = useState<string | null>(ARTIFACT_FOR_STEP[step] ?? "spec.md");
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editing, setEditing] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    const forStep = ARTIFACT_FOR_STEP[step];
    if (!forStep) return;
    if (step === "checklist" && checklists.length > 0) {
      setRel(`checklists/${checklists[0]}`);
    } else {
      setRel(forStep);
    }
  }, [step, checklists]);

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

  const save = useCallback(async () => {
    if (!projectRoot || !rel || content === null || !dirty) return;
    setSaving(true);
    try {
      await invoke("write_artifact", {
        projectRoot,
        featureSlug: rel.startsWith(".specify") ? null : featureSlug,
        relPath: rel,
        content,
      });
      setDirty(false);
      toast.success("Artifact saved", { description: rel });
    } catch (e) {
      setError(String(e));
      toast.error("Save failed", { description: String(e) });
    } finally {
      setSaving(false);
    }
  }, [projectRoot, featureSlug, rel, content, dirty]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [save]);

  const copyContent = async () => {
    if (content === null) return;
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch { /* clipboard unavailable */ }
  };

  const isChecklist = rel?.includes("checklists/") ?? false;
  const checkItems = isChecklist && content !== null
    ? content.split("\n").map((line, i) => {
        const m = line.match(/^(\s*[-*]\s*)\[( |x|X)\]\s*(.*)$/);
        return m ? { line: i, checked: m[2].toLowerCase() === "x", text: m[3] } : null;
      }).filter((x): x is { line: number; checked: boolean; text: string } => x !== null)
    : [];
  const checkedCount = checkItems.filter((c) => c.checked).length;

  return (
    <aside className="flex h-full flex-col bg-card/30">
      <header className="flex h-12 shrink-0 items-center gap-0.5 border-b border-border px-2" title="Artifacts — documents the agent writes land here for review">
        {TABS.map((t) => (
          <button
            key={t.rel}
            onClick={() => setRel(t.rel)}
            className={cn(
              "rounded px-2.5 py-1 font-mono text-xs transition-colors",
              rel === t.rel
                ? "bg-secondary text-foreground"
                : "text-muted-foreground/70 hover:text-foreground"
            )}
          >
            {t.label}
          </button>
        ))}
        <span className="flex-1" />
        {content !== null && (
          <>
            <Button variant="ghost" size="icon" className="h-6 w-6" title="Copy content" onClick={copyContent}>
              {copied ? <Check className="h-3 w-3 text-primary" /> : <Copy className="h-3 w-3" />}
            </Button>
            {!isChecklist && (
              <Button
                variant="ghost"
                size="icon"
                className="h-6 w-6"
                title={editing ? "Preview" : "Edit"}
                onClick={() => setEditing((v) => !v)}
              >
                {editing ? <Eye className="h-3 w-3" /> : <Pencil className="h-3 w-3" />}
              </Button>
            )}
          </>
        )}
        {editing && (
          <Button
            size="sm"
            variant={dirty ? "default" : "secondary"}
            className="h-6 px-2 text-[11px]"
            disabled={!dirty || saving}
            onClick={save}
          >
            {saving ? <Loader2 className="h-3 w-3 animate-spin" /> : "Save"}
          </Button>
        )}
        {dirty && <Badge variant="warning" className="px-1.5 text-[9px]">unsaved</Badge>}
      </header>

      <div className="flex flex-1 flex-col overflow-hidden">
        {!projectRoot ? (
          <EmptyState
            icon={FolderOpen}
            title="No project open"
            body="This panel is the review surface: documents the agent writes — spec, plan, tasks, checklists — land here for you to read, edit, and approve."
          />
        ) : content !== null && isChecklist ? (
          <div className="flex flex-1 flex-col overflow-hidden">
            <div className="shrink-0 border-b border-border/50 px-4 py-2.5">
              <div className="mb-1.5 flex items-center justify-between">
                <span className="flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                  <ClipboardCheck className="h-3 w-3" />
                  Reviewer checklist — only you can flip these
                </span>
                <span className="font-mono text-[10px] text-muted-foreground">
                  {checkedCount}/{checkItems.length}
                </span>
              </div>
              <Progress value={checkItems.length > 0 ? (checkedCount / checkItems.length) * 100 : 0} />
            </div>
            <ScrollArea className="flex-1 px-4 py-3">
              {content.split("\n").map((line, i) => {
                const m = line.match(/^(\s*[-*]\s*)\[( |x|X)\]\s*(.*)$/);
                if (!m) {
                  return line.trim() ? (
                    <p key={i} className="whitespace-pre-wrap text-[11px] leading-relaxed text-muted-foreground">{line}</p>
                  ) : (
                    <div key={i} className="h-1.5" />
                  );
                }
                const checked = m[2].toLowerCase() === "x";
                return (
                  <label key={i} className="group flex cursor-pointer items-start gap-2.5 rounded px-1 py-1 transition-colors hover:bg-secondary/50">
                    <Checkbox
                      checked={checked}
                      className="mt-0.5"
                      onCheckedChange={async (v) => {
                        try {
                          await invoke("flip_checklist_item", {
                            projectRoot,
                            featureSlug,
                            relPath: rel,
                            lineIndex: i,
                            checked: v === true,
                          });
                          await load();
                        } catch (err) {
                          toast.error("Flip failed", { description: String(err) });
                        }
                      }}
                    />
                    <span className={cn("text-xs leading-relaxed", checked ? "text-muted-foreground/70 line-through" : "text-foreground/85")}>
                      {m[3]}
                    </span>
                  </label>
                );
              })}
              <div className="mt-4 flex justify-end pb-2">
                <Button variant="ghost" size="sm" className="text-[11px]" onClick={() => setRel("spec.md")}>
                  ← back to artifacts
                </Button>
              </div>
            </ScrollArea>
          </div>
        ) : content !== null && editing ? (
          <ScrollArea className="flex-1">
            <textarea
              value={content}
              onChange={(e) => {
                setContent(e.target.value);
                setDirty(true);
              }}
              spellCheck={false}
              className="min-h-full w-full resize-none bg-transparent px-4 py-3 font-mono text-xs leading-relaxed text-foreground/85 outline-none"
            />
          </ScrollArea>
        ) : content !== null ? (
          <ScrollArea className="flex-1">
            <div className="px-4 py-3 artifact-markdown">
              <Markdown>{content}</Markdown>
            </div>
          </ScrollArea>
        ) : (
          error?.includes("no such file") || error?.includes("No such file") || error?.includes("no feature") ? (
            <EmptyState
              icon={FileText}
              title={`${rel} doesn't exist yet`}
              body={`Agent output lands here. It will appear after the ${
                rel === "spec.md" ? "specify" : rel === "plan.md" ? "plan" : "tasks"
              } step runs — then you can review and edit it directly.`}
            />
          ) : error ? (
            <EmptyState icon={FileText} title="Couldn't load artifact" body={error} />
          ) : (
            <div className="flex h-full items-center justify-center">
              <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
            </div>
          )
        )}
      </div>
    </aside>
  );
}
