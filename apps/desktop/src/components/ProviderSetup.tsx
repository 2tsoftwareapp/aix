import { useState } from "react";
import { Eye, EyeOff, KeyRound, Loader2, ShieldCheck, CheckCircle2, CircleDashed, Trash2, ChevronDown } from "lucide-react";
import { Dialog, DialogContent, DialogTitle, DialogDescription } from "./ui/dialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { toast } from "sonner";
import { cn } from "../lib/utils";
import { useKeyStatus, storeKey, deleteKey } from "../lib/providerKeys";

const PROVIDERS = [
  { id: "anthropic", label: "Anthropic", hint: "sk-ant-…", source: "console.anthropic.com" },
  { id: "openai", label: "OpenAI", hint: "sk-…", source: "platform.openai.com" },
  { id: "openrouter", label: "OpenRouter", hint: "sk-or-…", source: "openrouter.ai" },
  { id: "qwen", label: "Qwen", hint: "sk-… (DashScope)", source: "bailian.console.aliyun.com" },
];

export function ProviderSetup({ onDone }: { onDone: (stored: boolean) => void }) {
  const keyStatus = useKeyStatus();
  const [anyStored, setAnyStored] = useState(false);

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onDone(anyStored); }}>
      <DialogContent className="max-w-md">
        <div className="mb-1 flex items-center gap-2">
          <KeyRound className="h-4 w-4 text-primary" />
          <DialogTitle>Provider settings</DialogTitle>
        </div>
        <DialogDescription className="flex items-start gap-1.5 pb-3">
          <ShieldCheck className="mt-0.5 h-3 w-3 shrink-0" />
          Keys are stored in ~/.config/aix/credentials.json with owner-only permissions. Environment variables also work as fallback.
        </DialogDescription>

        <div className="space-y-2">
          {PROVIDERS.map((p) => (
            <ProviderRow
              key={p.id}
              meta={p}
              configured={!!keyStatus[p.id]}
              onStored={() => setAnyStored(true)}
            />
          ))}
        </div>

        <div className="flex justify-end pt-4">
          <Button size="sm" variant="secondary" onClick={() => onDone(anyStored)}>
            Done
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function ProviderRow({
  meta,
  configured,
  onStored,
}: {
  meta: (typeof PROVIDERS)[number];
  configured: boolean;
  onStored: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [key, setKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [justSaved, setJustSaved] = useState(false);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await storeKey(meta.id, key.trim());
      setKey("");
      setJustSaved(true);
      setOpen(false);
      onStored();
      toast.success(`${meta.label} key stored — provider active`);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    try {
      await deleteKey(meta.id);
      setJustSaved(false);
      toast.success(`${meta.label} key removed`);
    } catch (e) {
      toast.error(`remove failed: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={cn(
      "rounded-md border transition-colors",
      configured ? "border-primary/30 bg-primary/5" : "border-border"
    )}>
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2.5 px-3 py-2.5 text-left"
      >
        {configured ? (
          <CheckCircle2 className="h-4 w-4 shrink-0 text-primary" />
        ) : (
          <CircleDashed className="h-4 w-4 shrink-0 text-muted-foreground/40" />
        )}
        <span className="flex-1 text-sm">{meta.label}</span>
        {justSaved && configured && (
          <span className="rounded bg-primary/15 px-1.5 py-0.5 text-[10px] font-medium text-primary">saved</span>
        )}
        <span className={cn("text-[11px]", configured ? "text-primary/80" : "text-muted-foreground/50")}>
          {configured ? "configured" : "no key"}
        </span>
        <ChevronDown className={cn("h-3.5 w-3.5 text-muted-foreground/50 transition-transform", open && "rotate-180")} />
      </button>

      {open && (
        <div className="border-t border-border/60 px-3 pb-3 pt-2.5">
          <div className="relative">
            <Input
              type={showKey ? "text" : "password"}
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder={configured ? "paste a new key to replace…" : meta.hint}
              className="pr-9 font-mono text-xs"
              onKeyDown={(e) => { if (e.key === "Enter" && key.trim()) save(); }}
            />
            <button
              type="button"
              onClick={() => setShowKey((v) => !v)}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground/60 hover:text-foreground"
              aria-label={showKey ? "Hide key" : "Show key"}
            >
              {showKey ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
            </button>
          </div>
          <p className="pt-1.5 text-[11px] text-muted-foreground/70">
            Get a key at <span className="font-mono">{meta.source}</span>
          </p>
          {error && <p className="pt-2 text-xs text-destructive">{error}</p>}
          <div className="flex items-center justify-between pt-2.5">
            {configured ? (
              <Button variant="ghost" size="sm" disabled={busy} onClick={remove}
                className="text-destructive/80 hover:text-destructive">
                <Trash2 className="h-3 w-3" /> Remove key
              </Button>
            ) : <span />}
            <Button size="sm" disabled={!key.trim() || busy} onClick={save}>
              {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : configured ? "Replace key" : "Store key"}
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
