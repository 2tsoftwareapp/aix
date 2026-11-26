import { useState } from "react";
import { CheckCircle2, Trash2 } from "lucide-react";
import { useKeyStatus, storeKey, deleteKey } from "../lib/providerKeys";
import { useAppToast } from "../lib/toast";
import { HStack, VStack, StackItem, Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { Text } from "@astryxdesign/core/Text";
import { Icon } from "@astryxdesign/core/Icon";
import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Badge } from "@astryxdesign/core/Badge";
import { Card } from "@astryxdesign/core/Card";
import { TextInput } from "@astryxdesign/core/TextInput";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";

const PROVIDERS = [
  { id: "anthropic", display: "Anthropic", env: "ANTHROPIC_API_KEY", placeholder: "sk-ant-…", source: "console.anthropic.com" },
  { id: "openai", display: "OpenAI", env: "OPENAI_API_KEY", placeholder: "sk-…", source: "platform.openai.com" },
  { id: "openrouter", display: "OpenRouter", env: "OPENROUTER_API_KEY", placeholder: "sk-or-…", source: "openrouter.ai" },
  { id: "qwen", display: "Qwen", env: "DASHSCOPE_API_KEY", placeholder: "sk-… (DashScope)", source: "bailian.console.aliyun.com" },
] as const;

function ProviderCard({ provider, configured }: { provider: (typeof PROVIDERS)[number]; configured: boolean }) {
  const toast = useAppToast();
  const [draft, setDraft] = useState("");
  const [busyAction, setBusyAction] = useState<"save" | "remove" | null>(null);

  const save = async () => {
    const key = draft.trim();
    if (!key || busyAction) return;
    setBusyAction("save");
    try {
      await storeKey(provider.id, key);
      setDraft("");
      toast.success(`${provider.display} key stored`, "provider active");
    } catch (error) {
      toast.error("Couldn't store key", String(error));
    } finally {
      setBusyAction(null);
    }
  };

  const remove = async () => {
    if (busyAction) return;
    setBusyAction("remove");
    try {
      await deleteKey(provider.id);
      toast.success(`${provider.display} key removed`);
    } catch (error) {
      toast.error("Couldn't remove key", String(error));
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <Card variant="muted" padding={3} width="100%">
      <VStack gap={2} width="100%">
        <HStack justify="between" vAlign="center" width="100%">
          <Text type="label" weight="semibold">{provider.display}</Text>
          {configured && (
            <Badge
              variant="success"
              label="configured"
              icon={<Icon icon={CheckCircle2} size="xsm" />}
            />
          )}
        </HStack>
        <Text type="supporting" color="secondary">
          Get a key at <Text type="code" size="xsm" color="inherit">{provider.source}</Text> · env fallback: {provider.env}
        </Text>
        <HStack gap={2} width="100%" vAlign="end">
          <StackItem size="fill">
            <TextInput
              label={`${provider.display} API key`}
              isLabelHidden
              aria-label={`${provider.display} API key`}
              type="password"
              value={draft}
              onChange={setDraft}
              onEnter={() => void save()}
              placeholder={configured ? "paste a new key to replace…" : provider.placeholder}
              width="100%"
            />
          </StackItem>
          <Button
            label={configured ? "Replace" : "Store"}
            variant="secondary"
            isDisabled={!draft.trim()}
            isLoading={busyAction === "save"}
            clickAction={save}
          />
          {configured && (
            <IconButton
              label={`Remove ${provider.display} key`}
              variant="ghost"
              icon={<Icon icon={Trash2} size="sm" color="error" />}
              isLoading={busyAction === "remove"}
              clickAction={remove}
            />
          )}
        </HStack>
      </VStack>
    </Card>
  );
}

export function ProviderSetup({ isOpen, onClose }: { isOpen: boolean; onClose: () => void }) {
  const keyStatus = useKeyStatus();
  const hasConfiguredProvider = Object.values(keyStatus).some(Boolean);

  return (
    <Dialog
      isOpen={isOpen}
      onOpenChange={(open) => { if (!open) onClose(); }}
      width={560}
      maxHeight="82dvh"
      padding={0}
    >
      <Layout
        header={
          <DialogHeader
            title="Connect a provider"
            subtitle="Add an API key when you are ready to run agents."
            onOpenChange={(open) => { if (!open) onClose(); }}
          />
        }
        content={
          <LayoutContent isScrollable>
            <VStack gap={3} padding={3}>
              <Text type="supporting" color="secondary">
                aix uses provider API keys for agent runs. Signing in to a provider CLI does not configure this app.
                Keys are stored locally in ~/.config/aix/credentials.json with owner-only permissions.
              </Text>
              {PROVIDERS.map((provider) => (
                <ProviderCard key={provider.id} provider={provider} configured={!!keyStatus[provider.id]} />
              ))}
            </VStack>
          </LayoutContent>
        }
        footer={
          <LayoutFooter hasDivider>
            <HStack justify="end" width="100%">
              <Button label={hasConfiguredProvider ? "Done" : "Skip for now"} variant="secondary" onClick={onClose} />
            </HStack>
          </LayoutFooter>
        }
      />
    </Dialog>
  );
}
