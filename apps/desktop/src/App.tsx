import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { Toaster, toast } from "sonner";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { ArtifactPanel } from "./components/ArtifactPanel";
import { ProviderSetup } from "./components/ProviderSetup";
import type { PipelineState, StepId } from "./types";
import { addRecent, getLastProject } from "./lib/recents";

export function App() {
  const [activeStep, setActiveStep] = useState<StepId>("specify");
  const [projectRoot, setProjectRoot] = useState<string | null>(null);
  const [pipeline, setPipeline] = useState<PipelineState | null>(null);
  const [activeFeature, setActiveFeature] = useState<string | null>(null);
  const [hasKey, setHasKey] = useState(false);
  const [keyVersion, setKeyVersion] = useState(0);
  const [showSetup, setShowSetup] = useState(false);
  const [refreshToken, setRefreshToken] = useState(0);

  const refreshPipeline = useCallback(
    async (root: string, selectNewest = false) => {
      try {
        const state = await invoke<PipelineState>("scan_pipeline", { projectRoot: root });
        setPipeline(state);
        if (state.features.length > 0) {
          const stillExists = activeFeature && state.features.some((f) => f.slug === activeFeature);
          if (!stillExists || selectNewest) {
            setActiveFeature(state.features[state.features.length - 1].slug);
          }
        }
      } catch (e) {
        toast.error("Project scan failed", { description: String(e) });
      }
    },
    [activeFeature]
  );

  useEffect(() => {
    // any configured provider counts — not just anthropic
    Promise.all(
      ["anthropic", "openai", "openrouter", "qwen"].map((p) =>
        invoke<boolean>("has_provider_key", { providerId: p }).catch(() => false)
      )
    ).then((results) => {
      const any = results.some(Boolean);
      setHasKey(any);
      if (!any) setShowSetup(true);
    });
    const last = getLastProject();
    if (last) setProjectRoot(last);
  }, []);

  const openProject = useCallback((path: string) => {
    addRecent(path);
    setActiveFeature(null);
    setProjectRoot(path);
  }, []);

  useEffect(() => {
    if (projectRoot) refreshPipeline(projectRoot);
  }, [projectRoot, refreshPipeline]);

  return (
    <Tooltip0>
      <div className="flex h-full bg-background text-foreground">
        <PanelGroup direction="horizontal" autoSaveId="aix-layout">
          <Panel defaultSize={18} minSize={14} maxSize={28}>
            <Sidebar
              activeStep={activeStep}
              onSelectStep={setActiveStep}
              projectRoot={projectRoot}
              onOpenProject={openProject}
              pipeline={pipeline}
              activeFeature={activeFeature}
              onSelectFeature={setActiveFeature}
              onOpenSetup={() => setShowSetup(true)}
            />
          </Panel>
          <PanelResizeHandle className="w-px bg-border transition-colors hover:bg-primary/40 data-[resize-handle-state=drag]:bg-primary" />
          <Panel defaultSize={52} minSize={32}>
            <ChatPanel
              step={activeStep}
              projectRoot={projectRoot}
              featureSlug={activeFeature}
              hasKey={keyVersion}
              onStepDone={() => {
                if (projectRoot) {
                  // specify always creates a new feature dir — jump to it
                  refreshPipeline(projectRoot, activeStep === "specify");
                }
                setRefreshToken((t) => t + 1);
              }}
            />
          </Panel>
          <PanelResizeHandle className="w-px bg-border transition-colors hover:bg-primary/40 data-[resize-handle-state=drag]:bg-primary" />
          <Panel defaultSize={30} minSize={20} maxSize={45}>
            <ArtifactPanel
              step={activeStep}
              projectRoot={projectRoot}
              featureSlug={activeFeature}
              refreshToken={refreshToken}
              checklists={
                pipeline?.features.find((f) => f.slug === activeFeature)?.artifacts.checklists.map((c) => c.name) ?? []
              }
            />
          </Panel>
        </PanelGroup>
        {showSetup && (
          <ProviderSetup
            onDone={(stored) => {
              setShowSetup(false);
              if (stored) {
                setHasKey(true);
                // boolean hasKey can't re-fire effects when already true —
                // a version counter guarantees the key-status refresh runs
                setKeyVersion((v) => v + 1);
              }
            }}
          />
        )}
        <Toaster theme="dark" position="bottom-right" toastOptions={{
          style: { background: "hsl(var(--card))", border: "1px solid hsl(var(--border))", color: "hsl(var(--foreground))" },
        }} />
      </div>
    </Tooltip0>
  );
}

// lightweight wrapper so tooltips work app-wide without circular imports
import { TooltipProvider } from "./components/ui/tooltip";
function Tooltip0({ children }: { children: React.ReactNode }) {
  return <TooltipProvider delayDuration={300}>{children}</TooltipProvider>;
}
