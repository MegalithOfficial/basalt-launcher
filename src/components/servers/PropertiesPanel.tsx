import { useEffect, useMemo, useState } from "react";
import { Loader2, Plus, RotateCw, Search, Trash2 } from "lucide-react";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { configFile } from "../../lib/servers";
import type { Server, ServerProperty } from "../../lib/types";
import { Select } from "../Select";
import { Toggle } from "../ui";
import { useStore } from "../../store";

type Control =
  | { kind: "toggle" }
  | { kind: "number" }
  | { kind: "text" }
  | { kind: "choice"; options: string[] };

const CONTROLS: Record<string, Control> = {
  "server-port": { kind: "number" },
  "max-players": { kind: "number" },
  "view-distance": { kind: "number" },
  "simulation-distance": { kind: "number" },
  "spawn-protection": { kind: "number" },
  "player-idle-timeout": { kind: "number" },
  "rate-limit": { kind: "number" },
  "query.port": { kind: "number" },
  "rcon.port": { kind: "number" },
  "max-world-size": { kind: "number" },
  "entity-broadcast-range-percentage": { kind: "number" },
  "op-permission-level": { kind: "number" },
  "function-permission-level": { kind: "number" },
  "network-compression-threshold": { kind: "number" },
  gamemode: { kind: "choice", options: ["survival", "creative", "adventure", "spectator"] },
  difficulty: { kind: "choice", options: ["peaceful", "easy", "normal", "hard"] },
  "level-type": {
    kind: "choice",
    options: ["minecraft:normal", "minecraft:flat", "minecraft:large_biomes", "minecraft:amplified"],
  },
  pvp: { kind: "toggle" },
  hardcore: { kind: "toggle" },
  "online-mode": { kind: "toggle" },
  "white-list": { kind: "toggle" },
  "enforce-whitelist": { kind: "toggle" },
  "spawn-monsters": { kind: "toggle" },
  "spawn-animals": { kind: "toggle" },
  "spawn-npcs": { kind: "toggle" },
  "allow-flight": { kind: "toggle" },
  "allow-nether": { kind: "toggle" },
  "enable-command-block": { kind: "toggle" },
  "enable-query": { kind: "toggle" },
  "enable-rcon": { kind: "toggle" },
  "enable-status": { kind: "toggle" },
  "force-gamemode": { kind: "toggle" },
  "sync-chunk-writes": { kind: "toggle" },
  "prevent-proxy-connections": { kind: "toggle" },
  "hide-online-players": { kind: "toggle" },
  "require-resource-pack": { kind: "toggle" },
  "log-ips": { kind: "toggle" },
  "use-native-transport": { kind: "toggle" },
};

const BOOLEAN = /^(true|false)$/i;

function controlFor(key: string, value: string): Control {
  const known = CONTROLS[key];
  if (known) return known;
  const trimmed = value.trim();
  if (BOOLEAN.test(trimmed)) return { kind: "toggle" };
  if (/^-?\d+$/.test(trimmed)) return { kind: "number" };
  return { kind: "text" };
}

export function PropertiesPanel({ server, live }: { server: Server; live: boolean }) {
  const refreshServers = useStore((s) => s.refreshServers);
  const software = useStore((s) => s.serverSoftware);
  const file = configFile(software, server.flavor);
  const fixedKeys = file.endsWith(".toml");

  const [properties, setProperties] = useState<ServerProperty[]>([]);
  const [edited, setEdited] = useState<Record<string, string>>({});
  const [removed, setRemoved] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const [newKey, setNewKey] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      setProperties(await api.getServerProperties(server.id));
      setEdited({});
      setRemoved([]);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, [server.id]);

  const stored = useMemo(
    () => Object.fromEntries(properties.map((entry) => [entry.key, entry.value])),
    [properties],
  );
  const value = (key: string) => edited[key] ?? stored[key] ?? "";
  const changed = Object.keys(edited).length + removed.length;

  const rows = properties.filter(
    (entry) =>
      !removed.includes(entry.key) &&
      entry.key.toLowerCase().includes(query.trim().toLowerCase()),
  );

  const set = (key: string, next: string) =>
    setEdited((current) => ({ ...current, [key]: next }));

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const changes = Object.entries(edited).map(([key, value]) => ({ key, value }));
      setProperties(await api.setServerProperties(server.id, changes, removed));
      setEdited({});
      setRemoved([]);
      await refreshServers();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setSaving(false);
    }
  };

  const addKey = () => {
    const key = newKey.trim();
    if (!key) return;
    set(key, "");
    setProperties((current) =>
      current.some((entry) => entry.key === key) ? current : [...current, { key, value: "" }],
    );
    setNewKey("");
  };

  if (loading) {
    return (
      <div className="grid flex-1 place-items-center text-content-faint">
        <Loader2 className="size-5 animate-spin" />
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-2 px-8 py-3">
        <div className="relative w-full max-w-sm">
          <Search className="absolute left-3 top-1/2 size-3.5 -translate-y-1/2 text-content-faint" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Filter keys"
            className="w-full rounded-lg border border-border bg-void py-1.5 pl-8 pr-3 text-[12px] text-content outline-none focus:border-(--accent)"
          />
        </div>

        <span className="font-pixel text-[10px] uppercase tracking-[0.28em] text-content-faint">
          {rows.length} keys in {file}
        </span>

        <div className="ml-auto flex items-center gap-2">
          {changed > 0 && live && (
            <span className="font-pixel text-[10px] uppercase tracking-[0.22em] text-warn">
              restart to apply
            </span>
          )}
          <button
            onClick={() => void load()}
            title="Reload from disk"
            className="grid size-8 place-items-center rounded-lg border border-border bg-surface-2 text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
          >
            <RotateCw className="size-3.5" />
          </button>
          <button
            onClick={() => void save()}
            disabled={changed === 0 || saving}
            className="inline-flex items-center gap-2 rounded-lg bg-(--accent) px-3.5 py-1.5 text-[12px] font-semibold text-black transition-opacity disabled:cursor-not-allowed disabled:opacity-45"
          >
            {saving && <Loader2 className="size-3.5 animate-spin" />}
            {changed > 0 ? `Save ${changed}` : "Save"}
          </button>
        </div>
      </div>

      {error && (
        <div className="wrap-break-word border-y border-danger/30 bg-danger/10 px-8 py-2 text-[11px] text-danger">
          {error}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-8 pb-6">
        {rows.map((entry) => {
          const control = controlFor(entry.key, stored[entry.key] ?? "");
          const dirty = entry.key in edited;
          return (
            <div
              key={entry.key}
              className={cn(
                "group/row grid grid-cols-[minmax(0,22rem)_minmax(0,1fr)] items-center gap-4 border-b border-border-soft/50 py-1.5",
                dirty && "bg-(--accent)/5",
              )}
            >
              <span className="wrap-break-word font-mono text-[12px] text-content-muted">
                {entry.key}
              </span>

              <div className="flex items-center gap-2">
                {control.kind === "toggle" ? (
                  <Toggle
                    label={entry.key}
                    checked={value(entry.key).trim().toLowerCase() === "true"}
                    onChange={(next) => set(entry.key, next ? "true" : "false")}
                  />
                ) : control.kind === "choice" ? (
                  <Select
                    value={value(entry.key) || null}
                    options={control.options}
                    onChange={(next) => set(entry.key, next)}
                    compact
                  />
                ) : (
                  <input
                    value={value(entry.key)}
                    inputMode={control.kind === "number" ? "numeric" : undefined}
                    onChange={(event) => set(entry.key, event.target.value)}
                    className={cn(
                      "rounded-md border border-transparent bg-surface-2/40 px-2 py-1 font-mono text-[12px] text-content outline-none transition-colors hover:border-border focus:border-(--accent) focus:bg-void",
                      control.kind === "number" ? "w-24" : "w-full max-w-md",
                    )}
                  />
                )}

                {entry.key === "online-mode" &&
                  value(entry.key).trim().toLowerCase() === "false" && (
                    <span className="text-[11px] text-warn">
                      anyone can join under any name
                    </span>
                  )}

                <button
                  onClick={() => {
                    setRemoved((current) => [...current, entry.key]);
                    setEdited((current) => {
                      const next = { ...current };
                      delete next[entry.key];
                      return next;
                    });
                  }}
                  title="Remove this key"
                  className="ml-auto grid size-7 shrink-0 place-items-center rounded-md text-content-faint opacity-0 transition-colors hover:text-danger focus-visible:opacity-100 group-hover/row:opacity-100"
                >
                  <Trash2 className="size-3.5" />
                </button>
              </div>
            </div>
          );
        })}

        {!fixedKeys && (
        <div className="flex items-center gap-2 py-3">
          <input
            value={newKey}
            onChange={(event) => setNewKey(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && addKey()}
            placeholder="new-property"
            className="w-72 rounded-md border border-border bg-void px-2 py-1 font-mono text-[12px] text-content outline-none focus:border-(--accent)"
          />
          <button
            onClick={addKey}
            disabled={!newKey.trim()}
            className="grid size-7 place-items-center rounded-md border border-border bg-surface-2 text-content-muted transition-colors hover:bg-surface-3 hover:text-content disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Plus className="size-3.5" />
          </button>
        </div>
        )}
      </div>
    </div>
  );
}
