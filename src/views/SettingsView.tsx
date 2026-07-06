import { useEffect, useState } from "react";
import { Check, Save } from "lucide-react";

import { Button, PageHeader } from "../components/ui";
import { api } from "../lib/api";
import type { LauncherSettings } from "../lib/types";
import { useStore } from "../store";

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="flex items-center justify-between gap-6 border-b border-border-soft py-4 last:border-0">
      <div>
        <div className="text-sm font-medium text-content">{label}</div>
        {hint && <div className="mt-0.5 text-xs text-content-muted">{hint}</div>}
      </div>
      {children}
    </label>
  );
}

const inputCls =
  "w-32 rounded-lg border border-border bg-surface px-3 py-2 text-sm text-content outline-none transition-colors focus:border-lava";

export function SettingsView() {
  const settings = useStore((s) => s.settings);
  const [draft, setDraft] = useState<LauncherSettings | null>(settings);
  const [saved, setSaved] = useState(false);

  useEffect(() => setDraft(settings), [settings]);

  if (!draft) return null;

  const set = (patch: Partial<LauncherSettings>) => {
    setDraft({ ...draft, ...patch });
    setSaved(false);
  };

  const save = async () => {
    await api.updateSettings(draft);
    useStore.setState({ settings: draft });
    setSaved(true);
  };

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title="Settings"
        subtitle="Defaults applied to new launches."
        actions={
          <Button onClick={save}>
            {saved ? <Check className="size-4" /> : <Save className="size-4" />}
            {saved ? "Saved" : "Save"}
          </Button>
        }
      />

      <div className="flex-1 overflow-y-auto p-8">
        <div className="mx-auto max-w-2xl rounded-xl border border-border bg-surface-2 px-6">
          <Field label="Minimum memory" hint="JVM minimum heap (MB)">
            <input
              type="number"
              className={inputCls}
              value={draft.min_memory_mb}
              onChange={(e) => set({ min_memory_mb: Number(e.target.value) })}
            />
          </Field>
          <Field label="Maximum memory" hint="JVM maximum heap (MB)">
            <input
              type="number"
              className={inputCls}
              value={draft.max_memory_mb}
              onChange={(e) => set({ max_memory_mb: Number(e.target.value) })}
            />
          </Field>
          <Field label="Concurrent downloads" hint="Parallel files during installs">
            <input
              type="number"
              className={inputCls}
              value={draft.concurrent_downloads}
              onChange={(e) => set({ concurrent_downloads: Number(e.target.value) })}
            />
          </Field>
          <Field label="Java path" hint="Leave empty to auto-detect or manage Java">
            <input
              type="text"
              placeholder="auto"
              className={`${inputCls} w-64`}
              value={draft.java_path ?? ""}
              onChange={(e) => set({ java_path: e.target.value || null })}
            />
          </Field>
          <Field
            label="CurseForge API key"
            hint="Needed for CurseForge search, free at console.curseforge.com"
          >
            <input
              type="password"
              placeholder="not set"
              className={`${inputCls} w-64`}
              value={draft.curseforge_api_key ?? ""}
              onChange={(e) => set({ curseforge_api_key: e.target.value || null })}
            />
          </Field>
        </div>
      </div>
    </div>
  );
}
