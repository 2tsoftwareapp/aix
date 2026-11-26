import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { ArtifactPanel } from "./components/ArtifactPanel";
import { ProviderSetup } from "./components/ProviderSetup";
import type { PipelineState, StepId } from "./types";

export function App() {
  const [activeStep, setActiveStep] = useState<StepId>("specify");
  const [projectRoot, setProjectRoot] = useState<string | null>(null);
  const [pipeline, setPipeline] = useState<PipelineState | null>(null);
  const [activeFeature, setActiveFeature] = useState<string | null>(null);
  const [hasKey, setHasKey] = useState(false);
  const [showSetup, setShowSetup] = useState(false);
  const [refreshToken, setRefreshToken] = useState(0);
  const [scanError, setScanError] = useState<string | null>(null);

  const refreshPipeline = useCallback(async (root: string) => {
    try {
      const state = await invoke<PipelineState>("scan_pipeline", { projectRoot: root });
      setPipeline(state);
      if (state.features.length > 0 && !activeFeature) {
        setActiveFeature(state.features[state.features.length - 1].slug);
      }
      setScanError(null);
    } catch (e) {
      setScanError(String(e));
    }
  }, [activeFeature]);

  useEffect(() => {
    invoke<boolean>("has_provider_key", { providerId: "anthropic" }).then((ok) => {
      setHasKey(ok);
      if (!ok) setShowSetup(true);
    });
  }, []);

  useEffect(() => {
    if (projectRoot) refreshPipeline(projectRoot);
  }, [projectRoot, refreshPipeline]);

  return (
    <div className="flex h-full bg-surface text-ink">
      <Sidebar
        activeStep={activeStep}
        onSelectStep={setActiveStep}
        projectRoot={projectRoot}
        onOpenProject={setProjectRoot}
        pipeline={pipeline}
        activeFeature={activeFeature}
        onSelectFeature={setActiveFeature}
        onOpenSetup={() => setShowSetup(true)}
      />
      <main className="flex flex-1 min-w-0">
        <ChatPanel
          step={activeStep}
          projectRoot={projectRoot}
          featureSlug={activeFeature}
          hasKey={hasKey}
          onStepDone={() => {
            if (projectRoot) refreshPipeline(projectRoot);
            setRefreshToken((t) => t + 1);
          }}
        />
        <ArtifactPanel
          step={activeStep}
          projectRoot={projectRoot}
          featureSlug={activeFeature}
          refreshToken={refreshToken}
        />
      </main>
      {scanError && (
        <div className="fixed bottom-4 left-64 right-auto z-40 max-w-md rounded-xl border border-red-400/40 bg-surface-raised px-4 py-3 text-xs text-red-300 shadow-lg">
          <div className="flex items-start justify-between gap-3">
            <span>Project scan failed: {scanError}</span>
            <button onClick={() => setScanError(null)} className="text-ink-faint hover:text-ink">✕</button>
          </div>
        </div>
      )}
      {showSetup && (
        <ProviderSetup
          onDone={() => {
            setShowSetup(false);
            setHasKey(true);
          }}
        />
      )}
    </div>
  );
}
