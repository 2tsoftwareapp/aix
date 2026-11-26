import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "../../lib/utils";

/**
 * Lightweight editable combobox: type any value or pick from suggestions.
 * Needed because provider model catalogs change faster than any hardcoded list.
 */
export function Combobox({
  value,
  onChange,
  options,
  className,
  inputClassName,
  disabled,
}: {
  value: string;
  onChange: (v: string) => void;
  options: string[];
  className?: string;
  inputClassName?: string;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState(value);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => setDraft(value), [value]);

  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onClick);
    return () => document.removeEventListener("mousedown", onClick);
  }, []);

  // only filter once the user actually edits — opening the dropdown shows everything
  const filtering = draft !== value;
  const filtered = filtering
    ? options.filter((o) => o.toLowerCase().includes(draft.toLowerCase()))
    : options;

  const commit = (v: string) => {
    onChange(v);
    setDraft(v);
    setOpen(false);
  };

  return (
    <div ref={ref} className={cn("relative", className)}>
      <div
        className={cn(
          "flex h-7 items-center rounded-md border border-input bg-background pr-1 transition-colors focus-within:ring-2 focus-within:ring-ring",
          disabled && "opacity-50 pointer-events-none"
        )}
      >
        <input
          value={draft}
          onChange={(e) => {
            setDraft(e.target.value);
            onChange(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          spellCheck={false}
          className={cn(
            "h-full w-full bg-transparent px-2 font-mono text-xs text-muted-foreground outline-none placeholder:text-muted-foreground/40",
            inputClassName
          )}
        />
        <button
          type="button"
          tabIndex={-1}
          onClick={() => setOpen((v) => !v)}
          className="shrink-0 rounded p-0.5 text-muted-foreground/60 hover:text-foreground"
        >
          <ChevronDown className="h-3.5 w-3.5" />
        </button>
      </div>
      {open && filtered.length > 0 && (
        <div className="absolute right-0 z-50 mt-1 max-h-56 min-w-full w-max overflow-y-auto rounded-md border border-border bg-popover p-1 shadow-lg animate-fade-in">
          {filtered.map((o) => (
            <button
              key={o}
              onClick={() => commit(o)}
              className="flex w-full items-center justify-between gap-3 rounded-sm px-2 py-1.5 text-left font-mono text-xs text-muted-foreground transition-colors hover:bg-secondary hover:text-foreground"
            >
              {o}
              {o === value && <Check className="h-3 w-3 text-primary" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
