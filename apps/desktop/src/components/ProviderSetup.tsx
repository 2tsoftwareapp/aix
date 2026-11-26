import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

const PROVIDERS = [
  { id: "anthropic", label: "Anthropic", hint: "console.anthropic.com API key" },
  { id: "openai", label: "OpenAI", hint: "platform.openai.com API key" },
  { id: "openrouter", label: "OpenRouter", hint: "openrouter.ai API key" },
];

export function ProviderSetup({ onDone }: { onDone: () => void }) {
  const [provider, setProvider] = useState("anthropic");
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke("store_provider_key", { providerId: provider, key: key.trim() });
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="w-[26rem] rounded-xl border border-edge bg-surface-raised p-6">
        <h2 className="text-lg font-medium mb-1">Connect a provider</h2>
        <p className="text-xs text-ink-faint mb-4">
          Keys are stored in your OS keychain — never in files or logs.
        </p>
        <div className="flex gap-2 mb-4">
          {PROVIDERS.map((p) => (
            <button
              key={p.id}
              onClick={() => setProvider(p.id)}
              className={`flex-1 rounded-lg border px-3 py-2 text-sm ${
                provider === p.id
                  ? "border-accent text-ink bg-surface-overlay"
                  : "border-edge text-ink-dim hover:text-ink"
              }`}
            >
              {p.label}
            </button>
          ))}
        </div>
        <input
          type="password"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder={PROVIDERS.find((p) => p.id === provider)?.hint}
          className="w-full rounded-lg border border-edge bg-surface px-3 py-2 text-sm font-mono outline-none focus:border-accent"
        />
        {error && <p className="mt-2 text-xs text-red-400">{error}</p>}
        <div className="mt-4 flex justify-end gap-2">
          <button
            onClick={save}
            disabled={!key.trim() || busy}
            className="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent-hover disabled:opacity-40"
          >
            {busy ? "Storing…" : "Store key"}
          </button>
        </div>
      </div>
    </div>
  );
}
