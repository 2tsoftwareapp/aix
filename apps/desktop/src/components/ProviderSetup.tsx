import { useEffect, useMemo, useState } from "react";
import { CheckCircle2, Trash2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useKeyStatus, useVendorConfigs, storeKey, deleteKey } from "../lib/providerKeys";
import { useAppToast } from "../lib/toast";
import { HStack, VStack, StackItem, Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { Icon } from "@astryxdesign/core/Icon";
import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Badge } from "@astryxdesign/core/Badge";
import { TextInput } from "@astryxdesign/core/TextInput";
import { Typeahead, type SearchSource } from "@astryxdesign/core/Typeahead";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";

interface VendorItem {
  id: string;
  label: string;
  direct: boolean;
}

interface EmbeddingSettings {
  provider_id: string | null;
  model: string | null;
}

function prettify(id: string): string {
  return id.split("-").map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ");
}

export function ProviderSetup({ isOpen, onClose }: { isOpen: boolean; onClose: () => void }) {
  const keyStatus = useKeyStatus();
  const vendors = useVendorConfigs();
  const toast = useAppToast();

  const [vendorId, setVendorId] = useState<string | null>(null);
  const [, setVendorQuery] = useState("");
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [embeddingProviderId, setEmbeddingProviderId] = useState<string | null>(null);
  const [embeddingModel, setEmbeddingModel] = useState("");
  const [embeddingBusy, setEmbeddingBusy] = useState(false);
  const [, setEmbeddingQuery] = useState("");

  const vendor = vendors.find((v) => v.id === vendorId) ?? null;
  const configuredVendors = vendors.filter((v) => keyStatus[v.id]);

  // all upstream vendors in the live OpenRouter catalog — most of them are
  // only reachable *through* OpenRouter, but users think in vendor names,
  // so the selector should list everything and say so honestly
  const [catalogVendorIds, setCatalogVendorIds] = useState<string[]>([]);
  useEffect(() => {
    if (!isOpen) return;
    invoke<{ id: string }[]>("list_models", { providerId: "openrouter" })
      .then((models) => {
        const ids = [...new Set(
          models.filter((m) => m.id.includes("/")).map((m) => m.id.split("/")[0])
        )].sort();
        setCatalogVendorIds(ids);
      })
      .catch(() => { /* offline — curated list still works */ });
  }, [isOpen]);

  const vendorSource = useMemo<SearchSource<VendorItem>>(() => {
    const directIds = new Set(vendors.map((v) => v.id));
    const items: VendorItem[] = [
      ...vendors.map((v) => ({ id: v.id, label: v.display, direct: true })),
      ...catalogVendorIds
        .filter((id) => !directIds.has(id))
        .map((id) => ({ id, label: prettify(id), direct: false })),
    ].sort((a, b) =>
      Number(!!keyStatus[b.id]) - Number(!!keyStatus[a.id])
      || Number(b.direct) - Number(a.direct)
      || a.label.localeCompare(b.label));
    const match = (query: string) => {
      const needle = query.trim().toLowerCase();
      return items.filter((i) => !needle || i.label.toLowerCase().includes(needle) || i.id.includes(needle));
    };
    return { search: match, bootstrap: () => items };
  }, [vendors, catalogVendorIds, keyStatus]);

  const selectedVendorItem = useMemo<VendorItem | null>(() => (
    vendorId ? { id: vendorId, label: vendor?.display ?? prettify(vendorId), direct: !!vendor } : null
  ), [vendorId, vendor]);

  useEffect(() => {
    if (!isOpen) return;
    invoke<EmbeddingSettings>("get_embedding_settings")
      .then((settings) => {
        setEmbeddingProviderId(settings.provider_id);
        setEmbeddingModel(settings.model ?? "");
      })
      .catch((error) => toast.error("Couldn't load embedding settings", String(error)));
  }, [isOpen, toast]);

  const embeddingVendorSource = useMemo<SearchSource<VendorItem>>(() => {
    const items: VendorItem[] = vendors
      .filter((v) => v.kind === "openai_compat" && (keyStatus[v.id] || v.id === embeddingProviderId))
      .map((v) => ({ id: v.id, label: v.display, direct: true }))
      .sort((a, b) => a.label.localeCompare(b.label));
    const match = (query: string) => {
      const needle = query.trim().toLowerCase();
      return items.filter((i) => !needle || i.label.toLowerCase().includes(needle) || i.id.includes(needle));
    };
    return { search: match, bootstrap: () => items };
  }, [vendors, keyStatus, embeddingProviderId]);

  const selectedEmbeddingVendorItem = useMemo<VendorItem | null>(() => {
    if (!embeddingProviderId) return null;
    const configured = vendors.find((v) => v.id === embeddingProviderId);
    return { id: embeddingProviderId, label: configured?.display ?? prettify(embeddingProviderId), direct: true };
  }, [embeddingProviderId, vendors]);

  const saveEmbedding = async (providerId: string | null = embeddingProviderId, model: string = embeddingModel) => {
    if (embeddingBusy) return;
    setEmbeddingBusy(true);
    try {
      const saved = await invoke<EmbeddingSettings>("save_embedding_settings", {
        providerId,
        model: model.trim() || null,
      });
      setEmbeddingProviderId(saved.provider_id);
      setEmbeddingModel(saved.model ?? "");
      toast.success("Embedding settings saved", providerId || model.trim() ? "embedding override active" : "automatic embedding selection restored");
    } catch (error) {
      toast.error("Couldn't save embedding settings", String(error));
    } finally {
      setEmbeddingBusy(false);
    }
  };

  const save = async () => {
    const trimmed = key.trim();
    if (!vendor || !trimmed || busy) return;
    setBusy(true);
    try {
      await storeKey(vendor.id, trimmed);
      setKey("");
      toast.success(`${vendor.display} key stored`, "provider active");
    } catch (error) {
      toast.error("Couldn't store key", String(error));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (id: string, display: string) => {
    try {
      await deleteKey(id);
      toast.success(`${display} key removed`);
    } catch (error) {
      toast.error("Couldn't remove key", String(error));
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={(open) => { if (!open) onClose(); }}
      width={560}
      padding={4}
    >
      <Layout
        header={
          <DialogHeader
            title="Connect a provider"
            subtitle="Pick a vendor, paste its API key, save. Keys stay local in ~/.config/aix/credentials.json."
            onOpenChange={(open) => { if (!open) onClose(); }}
          />
        }
        content={
          <LayoutContent isScrollable>
            <VStack gap={3}>
              <HStack gap={2} width="100%" vAlign="end">
                <StackItem size="fill">
                  <Typeahead
                    label="Provider"
                    aria-label="Provider"
                    searchSource={vendorSource}
                    value={selectedVendorItem}
                    onChange={(item) => {
                      if (!item) return;
                      setVendorQuery("");
                      setVendorId(item.id);
                    }}
                    onChangeQuery={setVendorQuery}
                    renderItem={(item) => (
                      <HStack gap={2} vAlign="center" width="100%">
                        <Text type="body" size="sm">{item.label}</Text>
                        {keyStatus[item.id] && <Icon icon={CheckCircle2} size="xsm" color="success" />}
                        {!item.direct && (
                          <Text type="supporting" size="xsm" color="secondary">via OpenRouter</Text>
                        )}
                      </HStack>
                    )}
                    placeholder="Search provider…"
                    emptySearchText="No vendor matches"
                    hasEntriesOnFocus
                    hasClear={false}
                    maxMenuItems={12}
                    debounceMs={0}
                    size="md"
                    width="100%"
                  />
                </StackItem>
                <StackItem size="fill">
                  <TextInput
                    label="API key"
                    aria-label="API key"
                    type="password"
                    value={key}
                    onChange={setKey}
                    onEnter={() => void save()}
                    placeholder={vendor ? vendor.placeholder || "paste key…" : "select a provider first"}
                    isDisabled={!vendor}
                    width="100%"
                  />
                </StackItem>
                <Button
                  label="Save"
                  variant="primary"
                  isDisabled={!vendor || !key.trim() || !!(vendorId && !vendor)}
                  isLoading={busy}
                  clickAction={save}
                />
              </HStack>

              {vendor && (
                <Text type="supporting" size="xsm" color="secondary">
                  Get a key at <Text type="code" size="xsm" color="inherit">{vendor.source}</Text>
                  {" · "}env fallback: <Text type="code" size="xsm" color="inherit">{vendor.env}</Text>
                  {vendor.note ? ` · ${vendor.note}` : ""}
                </Text>
              )}
              {vendorId && !vendor && (
                <Text type="supporting" size="xsm" color="secondary">
                  {prettify(vendorId)} has no direct public API — its models are reachable through
                  OpenRouter{keyStatus.openrouter ? ", which you already have configured" : " once you store an OpenRouter key"}.
                  Nothing to configure here.
                </Text>
              )}

              <VStack gap={2} width="100%">
                <Text type="large" size="sm" weight="semibold">Embeddings for code search</Text>
                <HStack gap={2} width="100%" vAlign="end">
                  <StackItem size="fill">
                    <Typeahead
                      label="Provider override"
                      aria-label="Embedding provider override"
                      searchSource={embeddingVendorSource}
                      value={selectedEmbeddingVendorItem}
                      onChange={(item) => {
                        if (!item) return;
                        setEmbeddingQuery("");
                        setEmbeddingProviderId(item.id);
                      }}
                      onChangeQuery={setEmbeddingQuery}
                      renderItem={(item) => <Text type="body" size="sm">{item.label}</Text>}
                      placeholder="Automatic"
                      emptySearchText="No configured embedding provider matches"
                      hasEntriesOnFocus
                      hasClear={false}
                      maxMenuItems={8}
                      debounceMs={0}
                      size="md"
                      width="100%"
                    />
                  </StackItem>
                  <StackItem size="fill">
                    <TextInput
                      label="Model override"
                      aria-label="Embedding model override"
                      value={embeddingModel}
                      onChange={setEmbeddingModel}
                      placeholder="text-embedding-3-small"
                      width="100%"
                    />
                  </StackItem>
                </HStack>
                <HStack gap={2} justify="end" width="100%">
                  <Button
                    label="Automatic"
                    variant="ghost"
                    isDisabled={embeddingBusy || (!embeddingProviderId && !embeddingModel)}
                    clickAction={() => {
                      setEmbeddingProviderId(null);
                      setEmbeddingModel("");
                      void saveEmbedding(null, "");
                    }}
                  />
                  <Button
                    label="Save embeddings"
                    variant="secondary"
                    isLoading={embeddingBusy}
                    clickAction={() => void saveEmbedding()}
                  />
                </HStack>
                <Text type="supporting" size="xsm" color="secondary">
                  Blank means automatic provider/model selection. Environment variables AIX_EMBEDDING_PROVIDER and AIX_EMBEDDING_MODEL take precedence when set.
                </Text>
              </VStack>

              {configuredVendors.length > 0 && (
                <VStack gap={1} width="100%">
                  {configuredVendors.map((v) => (
                    <HStack key={v.id} justify="between" vAlign="center" width="100%">
                      <HStack gap={2} vAlign="center">
                        <Text type="body" size="sm">{v.display}</Text>
                        <Badge variant="success" label="configured" />
                      </HStack>
                      <IconButton
                        label={`Remove ${v.display} key`}
                        variant="ghost"
                        icon={<Icon icon={Trash2} size="sm" color="error" />}
                        clickAction={() => remove(v.id, v.display)}
                      />
                    </HStack>
                  ))}
                </VStack>
              )}
            </VStack>
          </LayoutContent>
        }
        footer={
          <LayoutFooter hasDivider>
            <HStack justify="end" width="100%">
              <Button label="Done" variant="secondary" onClick={onClose} />
            </HStack>
          </LayoutFooter>
        }
      />
    </Dialog>
  );
}
