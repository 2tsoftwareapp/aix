import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppShell } from "@astryxdesign/core/AppShell";
import { HStack, StackItem } from "@astryxdesign/core/Layout";
import { Card } from "@astryxdesign/core/Card";
import { ResizeHandle, useResizable } from "@astryxdesign/core/Resizable";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { ArtifactPanel } from "./components/ArtifactPanel";
import { ProviderSetup } from "./components/ProviderSetup";
import { useAppToast } from "./lib/toast";
import type { PipelineState, StepId } from "./types";
import { addRecent, getLastProject } from "./lib/recents";
import { chooseProjectDirectory } from "./lib/projectDialog";

export function App() {
  const toast = useAppToast();
  const [activeStep, setActiveStep] = useState<StepId>((localStorage.getItem("aix-step") as never) ?? "specify");
  const [projectRoot, setProjectRoot] = useState<string | null>(null);
  const [pipeline, setPipeline] = useState<PipelineState | null>(null);
  const [activeFeature, setActiveFeature] = useState<string | null>(null);
  const [showSetup, setShowSetup] = useState(false);
  const [refreshToken, setRefreshToken] = useState(0);
  const artifactResize = useResizable({
    defaultSize: 430,
    minSize: 320,
    maxSize: 720,
    autoSaveId: "aix-artifact-panel",
  });

  const refreshPipeline = useCallback(
    async (root: string, selectNewest = false) => {
      try {
        const state = await invoke<PipelineState>("scan_pipeline", { projectRoot: root });
        setPipeline(state);
        setActiveFeature((current) => {
          if (!selectNewest && current && state.features.some((f) => f.slug === current)) return current;
          const remembered = localStorage.getItem("aix-feature");
          const valid = remembered && state.features.some((f) => f.slug === remembered);
          return valid ? remembered : state.features[state.features.length - 1]?.slug ?? null;
        });
      } catch (e) {
        toast.error("Project scan failed", String(e));
      }
    },
    [toast]
  );

  useEffect(() => { localStorage.setItem("aix-step", activeStep); }, [activeStep]);
  useEffect(() => { if (activeFeature) localStorage.setItem("aix-feature", activeFeature); }, [activeFeature]);
  useEffect(() => {
    const last = getLastProject();
    if (last) setProjectRoot(last);
  }, []);

  const openProject = useCallback((path: string) => {
    addRecent(path);
    setActiveFeature(null);
    setProjectRoot(path);
  }, []);

  const openProjectPicker = useCallback(async () => {
    try {
      const path = await chooseProjectDirectory();
      if (path) openProject(path);
    } catch (error) {
      toast.error("Couldn't open project picker", String(error));
    }
  }, [openProject, toast]);

  useEffect(() => {
    if (projectRoot) refreshPipeline(projectRoot);
  }, [projectRoot, refreshPipeline]);

  const activeChecklists = useMemo(
    () => pipeline?.features.find((f) => f.slug === activeFeature)?.artifacts.checklists.map((c) => c.name) ?? [],
    [pipeline, activeFeature]
  );

  return (
    <>
      <AppShell
        height="fill"
        variant="surface"
        contentPadding={0}
        sideNav={
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
        }
      >
        <HStack height="100%" width="100%" gap={0}>
          <StackItem size="fill">
            <ChatPanel
              step={activeStep}
              projectRoot={projectRoot}
              featureSlug={activeFeature}
              onStepDone={() => {
                if (projectRoot) {
                  refreshPipeline(projectRoot, activeStep === "specify");
                }
                setRefreshToken((t) => t + 1);
              }}
              onOpenProject={openProjectPicker}
              onOpenSetup={() => setShowSetup(true)}
            />
          </StackItem>
          <ResizeHandle
            direction="horizontal"
            resizable={artifactResize.props}
            isReversed
            hasDivider
            label="Resize artifact panel"
          />
          <StackItem size="static" style={{ width: artifactResize.size }}>
            <Card variant="transparent" padding={0} width="100%" height="100%">
              <ArtifactPanel
                step={activeStep}
                projectRoot={projectRoot}
                featureSlug={activeFeature}
                refreshToken={refreshToken}
                checklists={activeChecklists}
              />
            </Card>
          </StackItem>
        </HStack>
      </AppShell>
      <ProviderSetup isOpen={showSetup} onClose={() => setShowSetup(false)} />
    </>
  );
}
