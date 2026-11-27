import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Brain, FileText, Search } from "lucide-react";
import { HStack, VStack, StackItem, Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { Icon } from "@astryxdesign/core/Icon";
import { Button } from "@astryxdesign/core/Button";
import { TextInput } from "@astryxdesign/core/TextInput";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Markdown } from "@astryxdesign/core/Markdown";
import { Spinner } from "@astryxdesign/core/Spinner";
import { useAppToast } from "../lib/toast";

interface MemoryHit {
  path: string;
  title: string;
  snippet: string;
}

interface MemoryPage {
  path: string;
  title: string;
  body: string;
}

function MemoryEmpty({ query, projectRoot }: { query: string; projectRoot: string | null }) {
  if (!projectRoot) {
    return (
      <EmptyState
        isCompact
        icon={<Icon icon={Brain} size="lg" color="secondary" />}
        title="No project open"
        description="Open a project before browsing its agent memory."
      />
    );
  }
  return (
    <EmptyState
      isCompact
      icon={<Icon icon={Search} size="lg" color="secondary" />}
      title={query.trim() ? "No memory found" : "Search project memory"}
      description={query.trim() ? "No memory page matches this query." : "Search decisions, lessons, and notes the agent wrote for this project."}
    />
  );
}

export function MemoryBrowser({ isOpen, onClose, projectRoot }: {
  isOpen: boolean;
  onClose: () => void;
  projectRoot: string | null;
}) {
  const toast = useAppToast();
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<MemoryHit[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [page, setPage] = useState<MemoryPage | null>(null);
  const [searching, setSearching] = useState(false);
  const [loadingPage, setLoadingPage] = useState(false);
  const generationRef = useRef(0);
  const pageGenerationRef = useRef(0);

  const search = useCallback(async (rawQuery: string) => {
    if (!projectRoot) return;
    const generation = ++generationRef.current;
    const trimmed = rawQuery.trim();
    if (!trimmed) {
      setHits([]);
      setSelectedPath(null);
      setPage(null);
      setSearching(false);
      return;
    }
    setSearching(true);
    try {
      const results = await invoke<MemoryHit[]>("search_memory_pages", {
        projectRoot,
        query: trimmed,
        limit: 20,
      });
      if (generationRef.current !== generation) return;
      setHits(results);
    } catch (error) {
      if (generationRef.current === generation) {
        toast.error("Memory search failed", String(error));
      }
    } finally {
      if (generationRef.current === generation) setSearching(false);
    }
  }, [projectRoot, toast]);

  useEffect(() => {
    if (!isOpen) return;
    const timer = window.setTimeout(() => void search(query), 250);
    return () => window.clearTimeout(timer);
  }, [isOpen, query, search]);

  useEffect(() => {
    setHits([]);
    setSelectedPath(null);
    setPage(null);
  }, [projectRoot]);

  const openPage = useCallback(async (path: string) => {
    if (!projectRoot) return;
    const generation = ++pageGenerationRef.current;
    setSelectedPath(path);
    setLoadingPage(true);
    try {
      const result = await invoke<MemoryPage | null>("read_memory_page", { projectRoot, path });
      if (pageGenerationRef.current === generation) setPage(result);
    } catch (error) {
      if (pageGenerationRef.current === generation) {
        setPage(null);
        toast.error("Couldn't read memory page", String(error));
      }
    } finally {
      if (pageGenerationRef.current === generation) setLoadingPage(false);
    }
  }, [projectRoot, toast]);

  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={(open) => { if (!open) onClose(); }}
      width={820}
      padding={4}
    >
      <Layout
        header={
          <DialogHeader
            title="Project memory"
            subtitle="Read-only view of durable pages the agent wrote for this project."
            onOpenChange={(open) => { if (!open) onClose(); }}
          />
        }
        content={
          <LayoutContent isScrollable={false}>
            <VStack gap={3} height="100%" width="100%">
              <TextInput
                label="Search memory"
                aria-label="Search memory"
                value={query}
                onChange={setQuery}
                onEnter={() => void search(query)}
                placeholder="decision, lesson, database…"
                width="100%"
                isDisabled={!projectRoot}
              />
              <HStack gap={3} width="100%" height="100%" style={{ minHeight: 420 }}>
                <StackItem size="static" style={{ width: 280 }}>
                  <VStack gap={1} width="100%" height="100%">
                    {searching && <HStack gap={2} vAlign="center"><Spinner size="sm" /><Text type="supporting" size="xsm" color="secondary">Searching…</Text></HStack>}
                    {!searching && hits.length === 0 && <MemoryEmpty query={query} projectRoot={projectRoot} />}
                    {hits.map((hit) => (
                      <button
                        key={hit.path}
                        type="button"
                        aria-label={`Open memory page ${hit.title}`}
                        onClick={() => void openPage(hit.path)}
                        style={{
                          textAlign: "left",
                          width: "100%",
                          border: "1px solid var(--color-border-secondary, rgba(128,128,128,.25))",
                          borderRadius: 8,
                          padding: "10px 12px",
                          background: selectedPath === hit.path ? "var(--color-bg-selected, rgba(80,120,255,.12))" : "transparent",
                          color: "inherit",
                          cursor: "pointer",
                        }}
                      >
                        <VStack gap={1}>
                          <HStack gap={2} vAlign="center">
                            <Icon icon={FileText} size="xsm" color="secondary" />
                            <Text type="body" size="sm" weight="medium" maxLines={1}>{hit.title}</Text>
                          </HStack>
                          <Text type="code" size="2xs" color="secondary" maxLines={1}>{hit.path}</Text>
                          <Text type="supporting" size="xsm" color="secondary" maxLines={3}>{hit.snippet}</Text>
                        </VStack>
                      </button>
                    ))}
                  </VStack>
                </StackItem>
                <StackItem size="fill">
                  <VStack width="100%" height="100%" style={{ borderLeft: "1px solid var(--color-border-secondary, rgba(128,128,128,.25))", paddingLeft: 16, overflow: "auto" }}>
                    {loadingPage && <HStack gap={2} vAlign="center"><Spinner size="sm" /><Text type="supporting" size="xsm" color="secondary">Loading page…</Text></HStack>}
                    {!loadingPage && page && (
                      <>
                        <VStack gap={0}>
                          <Text type="large" weight="semibold">{page.title}</Text>
                          <Text type="code" size="xsm" color="secondary">{page.path}</Text>
                        </VStack>
                        <Markdown density="compact">{page.body}</Markdown>
                      </>
                    )}
                    {!loadingPage && !page && hits.length > 0 && (
                      <EmptyState
                        isCompact
                        icon={<Icon icon={FileText} size="lg" color="secondary" />}
                        title="Select a memory page"
                        description="Choose a search result to read the full page."
                      />
                    )}
                  </VStack>
                </StackItem>
              </HStack>
            </VStack>
          </LayoutContent>
        }
        footer={
          <LayoutFooter hasDivider>
            <HStack justify="end" width="100%">
              <Button label="Close" variant="secondary" onClick={onClose} />
            </HStack>
          </LayoutFooter>
        }
      />
    </Dialog>
  );
}
