import { useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { ArtifactPanel } from "./components/ArtifactPanel";
import type { StepId } from "./types";

export function App() {
  const [activeStep, setActiveStep] = useState<StepId>("specify");
  const [artifactOpen, setArtifactOpen] = useState(true);

  return (
    <div className="flex h-full bg-surface text-ink">
      <Sidebar activeStep={activeStep} onSelectStep={setActiveStep} />
      <main className="flex flex-1 min-w-0">
        <ChatPanel step={activeStep} />
        {artifactOpen && (
          <ArtifactPanel step={activeStep} onClose={() => setArtifactOpen(false)} />
        )}
      </main>
    </div>
  );
}
