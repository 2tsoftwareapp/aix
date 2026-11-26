import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Eye, EyeOff, KeyRound, Loader2, ShieldCheck, CheckCircle2, CircleDashed } from "lucide-react";
import { Dialog, DialogContent, DialogTitle, DialogDescription } from "./ui/dialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { toast } from "sonner";
import { cn } from "../lib/utils";

const PROVIDERS = [
  { id: "anthropic", label: "Anthropic", hint: "sk-ant-…", source: "console.anthropic.com" },
  { id: "openai", label: "OpenAI", hint: "sk-…", source: "platform.openai.com" },
  { id: "openrouter", label: "OpenRouter", hint: "sk-or-…", source: "openrouter.ai" },
  { id: "qwen", label: "Qwen", hint: "sk-… (DashScope)", source: "bailian.console.aliyun.com" },
];

export function ProviderSetup({ onDone }: { onDone: (stored: boolean) => void }) {
  const [provider, setProvider] = useState("anthropic");
  const [key, setKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [configured, setConfigured] = useState<Record<string, boolean>>({});

  useEffect(() => {
    Promise.all(
      PROVIDERS.map(async (p) => [p.id, await invoke<boolean>("has_provider_key", { providerId: p.id })] as const)
    ).then((entries) => setConfigured(Object.fromEntries(entries)));
  }, []);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke("store_provider_key", { providerId: provider, key: key.trim() });
      setConfigured((c) => ({ ...c, [provider]: true }));
      toast.success(`${PROVIDERS.find((p) => p.id === provider)?.label} key stored`);
      onDone(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const current = PROVIDERS.find((p) => p.id === provider)!;

  return (
    <Dialog open onOpenChange={(o) => { if (!o) onDone(false); }}>
      <DialogContent>
        <div className="mb-1 flex items-center gap-2">
          <KeyRound className="h-4 w-4 text-primary" />
          <DialogTitle>Connect a provider</DialogTitle>
        </div>
        <DialogDescription className="flex items-start gap-1.5 pb-3">
          <ShieldCheck className="mt-0.5 h-3 w-3 shrink-0" />
          Keys are stored in your OS keychain — never in files or logs. Environment variables also work as fallback.
        </DialogDescription>

        <div className="grid grid-cols-2 gap-2 pb-4">
          {PROVIDERS.map((p) => (
            <button
              key={p.id}
              onClick={() => { setProvider(p.id); setError(null); }}
              className={cn(
                "flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors",
                provider === p.id
                  ? "border-primary/60 bg-primary/10 text-foreground"
                  : "border-border text-muted-foreground hover:border-border hover:bg-secondary hover:text-foreground"
              )}
            >
              {p.label}
              {configured[p.id] ? (
                <CheckCircle2 className="h-3.5 w-3.5 text-primary" />
              ) : (
                <CircleDashed className="h-3.5 w-3.5 text-muted-foreground/40" />
              )}
            </button>
          ))}
        </div>

        <div className="relative">
          <Input
            type={showKey ? "text" : "password"}
            value={key}
            onChange={(e) => setKey(e.target.value)}
            placeholder={current.hint}
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
          Get a key at <span className="font-mono">{current.source}</span>
        </p>

        {error && <p className="pt-2 text-xs text-destructive">{error}</p>}

        <div className="flex justify-end gap-2 pt-4">
          <Button variant="ghost" size="sm" onClick={() => onDone(false)}>Skip for now</Button>
          <Button size="sm" disabled={!key.trim() || busy} onClick={save}>
            {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : "Store key"}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
