import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppShell } from "@astryxdesign/core/AppShell";
import { HStack, VStack, StackItem } from "@astryxdesign/core/Layout";
import { Card } from "@astryxdesign/core/Card";
import { ResizeHandle, useResizable } from "@astryxdesign/core/Resizable";
import { IconButton } from "@astryxdesign/core/IconButton";
import { PanelRightOpen } from "lucide-react";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { ArtifactPanel } from "./components/ArtifactPanel";
import { ProviderSetup } from "./components/ProviderSetup";
import { MemoryBrowser } from "./components/MemoryBrowser";
import { useAppToast } from "./lib/toast";
import type { PipelineState, StepId } from "./types";
import type { WorkspaceContext } from "./lib/workspaceSessions";
import { addRecent, getLastProject } from "./lib/recents";
import { chooseProjectDirectory } from "./lib/projectDialog";

export function App() {
  const toast = useAppToast();
  const [activeStep, setActiveStep] = useState<StepId>((localStorage.getItem("aix-step") as never) ?? "specify");
  const [projectRoot, setProjectRoot] = useState<string | null>(null);
  const [pipeline, setPipeline] = useState<PipelineState | null>(null);
  const [activeFeature, setActiveFeature] = useState<string | null>(null);
  const [showSetup, setShowSetup] = useState(false);
  const [showMemory, setShowMemory] = useState(false);
  const [refreshToken, setRefreshToken] = useState(0);
  const artifactResize = useResizable({
    defaultSize: 430,
    minSize: 320,
    maxSize: 720,
    autoSaveId: "aix-artifact-panel",
  });
  const [artifactCollapsed, setArtifactCollapsed] = useState(() => localStorage.getItem("aix-artifact-collapsed") === "1");
  const [windowWidth, setWindowWidth] = useState(() => window.innerWidth);
  useEffect(() => {
    const onResize = () => setWindowWidth(window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);
  // Below this width the artifact panel would squeeze the chat under a usable
  // size, so it floats over the content instead of taking layout space.
  const artifactOverlay = windowWidth < 1150;
  const toggleArtifact = () => {
    setArtifactCollapsed((prev) => {
      localStorage.setItem("aix-artifact-collapsed", prev ? "0" : "1");
      return !prev;
    });
  };

  const refreshPipeline = useCallback(
    async (root: string, selectNewest = false) => {
      try {
        const state = await invoke<PipelineState>("scan_pipeline", { projectRoot: root });
        if (!state || !Array.isArray(state.features)) {
          throw new Error("scan_pipeline returned invalid pipeline state");
        }
        setPipeline(state);
        setActiveFeature((current) => {
          if (!selectNewest && current && state.features.some((f) => f.slug === current)) return current;
          if (selectNewest) return state.features[state.features.length - 1]?.slug ?? null;
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
    if (projectRoot && projectRoot !== path) {
      void invoke("close_project_lsp", { projectRoot }).catch((error) => {
        console.warn("LSP shutdown failed", error);
      });
    }
    addRecent(path);
    setActiveFeature(null);
    setProjectRoot(path);
  }, [projectRoot]);

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

  const handleStepDone = useCallback((doneContext: WorkspaceContext | null, completedFeatureSlug: string | null) => {
    if (projectRoot && (!doneContext || doneContext.projectRoot === projectRoot)) {
      const completedSpecify = doneContext?.step === "specify" && completedFeatureSlug != null;
      refreshPipeline(projectRoot, completedSpecify);
      if (completedSpecify) setActiveStep("specify");
    }
    setRefreshToken((t) => t + 1);
  }, [projectRoot, refreshPipeline]);

  return (
    <>
      <AppShell
        height="fill"
        variant="section"
        contentPadding={0}
        sideNav={
          <Sidebar
            projectRoot={projectRoot}
            onOpenProject={openProject}
            pipeline={pipeline}
            activeFeature={activeFeature}
            onSelectFeature={setActiveFeature}
            onOpenSetup={() => setShowSetup(true)}
            onOpenMemory={() => setShowMemory(true)}
          />
        }
      >
        <HStack height="100%" width="100%" gap={0} style={{ position: "relative" }}>
          <StackItem size="fill">
            <ChatPanel
              step={activeStep}
              pipeline={pipeline}
              onSelectStep={setActiveStep}
              projectRoot={projectRoot}
              featureSlug={activeFeature}
              onStepDone={handleStepDone}
              onOpenProject={openProjectPicker}
              onOpenSetup={() => setShowSetup(true)}
            />
          </StackItem>
          {!artifactCollapsed && !artifactOverlay && (
            <ResizeHandle
              direction="horizontal"
              resizable={artifactResize.props}
              isReversed
              hasDivider
              label="Resize artifact panel"
            />
          )}
          {artifactCollapsed ? (
            <StackItem size="static" style={{ width: 40 }}>
              <VStack padding={2} hAlign="center">
                <IconButton
                  icon={<PanelRightOpen size={14} />}
                  label="Expand artifact panel"
                  size="sm"
                  onClick={toggleArtifact}
                />
              </VStack>
            </StackItem>
          ) : artifactOverlay ? (
            <div
              className="aix-artifact-overlay"
              style={{
                position: "absolute", top: 0, right: 0, bottom: 0, zIndex: 20,
                width: Math.min(artifactResize.size, Math.max(320, windowWidth - 240)),
                background: "var(--color-background-popover, #1c1c1e)",
                borderLeft: "1px solid var(--color-border)",
                boxShadow: "-12px 0 32px rgba(0,0,0,0.35)",
              }}
            >
              <Card variant="transparent" padding={0} width="100%" height="100%">
                <ArtifactPanel
                  step={activeStep}
                  projectRoot={projectRoot}
                  featureSlug={activeFeature}
                  refreshToken={refreshToken}
                  checklists={activeChecklists}
                  onCollapse={toggleArtifact}
                />
              </Card>
            </div>
          ) : (
            <StackItem size="static" className="aix-artifact-panel" style={{ width: artifactResize.size }}>
              <Card variant="transparent" padding={0} width="100%" height="100%">
                <ArtifactPanel
                  step={activeStep}
                  projectRoot={projectRoot}
                  featureSlug={activeFeature}
                  refreshToken={refreshToken}
                  checklists={activeChecklists}
                  onCollapse={toggleArtifact}
                />
              </Card>
            </StackItem>
          )}
        </HStack>
      </AppShell>
      <ProviderSetup isOpen={showSetup} onClose={() => setShowSetup(false)} />
      <MemoryBrowser isOpen={showMemory} onClose={() => setShowMemory(false)} projectRoot={projectRoot} />
    </>
  );
}
