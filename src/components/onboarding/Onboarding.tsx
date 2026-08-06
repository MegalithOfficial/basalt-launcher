import { useCallback, useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowRight,
  Boxes,
  Check,
  Coffee,
  Compass,
  HardDrive,
  HardDriveDownload,
  Loader2,
  MemoryStick,
  TriangleAlert,
  UserCircle2,
} from "lucide-react";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import type {
  DataLocation,
  DataRoot,
  DiskInfo,
  JavaInfo,
  LauncherSettings,
  LauncherSource,
  SystemStats,
} from "../../lib/types";
import { useStore } from "../../store";
import { CreateInstanceModal } from "../CreateInstanceModal";
import { MemoryRange } from "../MemoryRange";
import { MigrateModal } from "../MigrateModal";
import { PlayerHead } from "../Avatar";
import { SignInModal } from "../SignInModal";
import { DataLocationsModal } from "../storage/DataLocationsModal";

type StepId = "welcome" | "storage" | "import" | "account" | "java" | "memory" | "done";

const BULK_SLOTS: DataRoot[] = [
  "instances",
  "versions",
  "libraries",
  "assets",
  "natives",
  "runtimes",
  "snapshots",
  "cache",
];

const TITLES: Record<StepId, { title: string; blurb: string }> = {
  welcome: {
    title: "Welcome to Basalt",
    blurb: "A fast, native Minecraft launcher. Four short steps and you are playing.",
  },
  storage: {
    title: "Where should the files go",
    blurb:
      "Modpacks and worlds run to tens of gigabytes. Put them on a roomier drive now and Basalt will not have to move them later.",
  },
  import: {
    title: "Bring your instances over",
    blurb: "Basalt found another launcher on this machine. Copying leaves the original untouched.",
  },
  account: {
    title: "Sign in with Microsoft",
    blurb: "Needed to launch the game. You can browse mods and packs without it.",
  },
  java: {
    title: "Java",
    blurb: "Minecraft runs on Java. Basalt uses what it finds unless you point it somewhere else.",
  },
  memory: {
    title: "How much memory",
    blurb: "The ceiling every instance inherits. Modpacks want more than vanilla does.",
  },
  done: {
    title: "You are set",
    blurb: "Make an instance of your own, or browse the modpacks people have already built.",
  },
};

function parentOf(path: string) {
  const cut = path.replace(/[/\\]+$/, "").lastIndexOf("/");
  return cut > 0 ? path.slice(0, cut) : path;
}

function diskLabel(disk: DiskInfo) {
  return disk.mount_point === "/" ? "system drive" : disk.mount_point;
}

function StepDots({ steps, at }: { steps: StepId[]; at: number }) {
  return (
    <div className="flex items-center gap-1.5">
      {steps.map((step, index) => (
        <span
          key={step}
          className={cn(
            "h-1 rounded-full transition-all duration-300",
            index === at
              ? "w-6 bg-(--accent)"
              : index < at
                ? "w-1.5 bg-(--accent)/40"
                : "w-1.5 bg-surface-3",
          )}
        />
      ))}
    </div>
  );
}

function Card({
  icon,
  title,
  hint,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  hint?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-3.5 rounded-2xl border border-border-soft bg-surface-2/60 px-4 py-3.5">
      <span className="grid size-10 shrink-0 place-items-center rounded-xl bg-surface-3 text-(--accent)">
        {icon}
      </span>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-medium text-content">{title}</div>
        {hint && <div className="mt-0.5 truncate text-xs text-content-faint">{hint}</div>}
      </div>
      {children}
    </div>
  );
}

export function Onboarding() {
  const settings = useStore((s) => s.settings);
  const accounts = useStore((s) => s.accounts);
  const instances = useStore((s) => s.instances);
  const addAccount = useStore((s) => s.addAccount);
  const setView = useStore((s) => s.setView);
  const openDiscover = useStore((s) => s.openDiscover);

  const [at, setAt] = useState(0);
  const [sources, setSources] = useState<LauncherSource[] | null>(null);
  const [javas, setJavas] = useState<JavaInfo[] | null>(null);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [memory, setMemory] = useState<{ min: number; max: number } | null>(null);
  const [migrateOpen, setMigrateOpen] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [finishing, setFinishing] = useState(false);
  const [javaMajors, setJavaMajors] = useState<number[]>([17, 21]);
  const [installingJava, setInstallingJava] = useState(false);
  const [javaError, setJavaError] = useState<string | null>(null);
  const [locations, setLocations] = useState<DataLocation[] | null>(null);
  const [movingData, setMovingData] = useState(false);
  const [storageError, setStorageError] = useState<string | null>(null);
  const [locationsOpen, setLocationsOpen] = useState(false);

  useEffect(() => {
    api.getDataLocations().then(setLocations).catch(() => setLocations([]));
    api.detectLaunchers().then(setSources).catch(() => setSources([]));
    api.listJavas().then(setJavas).catch(() => setJavas([]));
    api.getSystemStats().then(setStats).catch(() => setStats(null));
  }, []);

  useEffect(() => {
    const refreshJava = () => api.listJavas().then(setJavas).catch(() => {});
    window.addEventListener("focus", refreshJava);
    return () => window.removeEventListener("focus", refreshJava);
  }, []);

  useEffect(() => {
    if (settings && !memory) {
      setMemory({ min: settings.min_memory_mb, max: settings.max_memory_mb });
    }
  }, [settings, memory]);

  const steps = useMemo<StepId[]>(() => {
    const list: StepId[] = ["welcome", "account", "storage"];
    if (sources && sources.length > 0) list.push("import");
    if (javas && javas.length === 0) list.push("java");
    list.push("memory", "done");
    return list;
  }, [sources, javas]);

  const step = steps[Math.min(at, steps.length - 1)];
  const account = accounts.find((a) => a.active) ?? accounts[0];

  const instancesRoot = locations?.find((entry) => entry.slot === "instances");
  const dataBase = instancesRoot ? parentOf(instancesRoot.path) : null;
  const dataDisk = instancesRoot?.disk ?? null;

  const chooseDataFolder = async () => {
    const chosen = await openFolderDialog({
      directory: true,
      title: "Pick a folder for the Basalt game files",
    });
    if (typeof chosen !== "string" || !locations) return;
    setMovingData(true);
    setStorageError(null);
    try {
      for (const slot of BULK_SLOTS) {
        await api.setDataLocation(slot, `${chosen}/${slot}`, true);
      }
      setLocations(await api.getDataLocations());
    } catch (cause) {
      setStorageError(String(cause));
      setLocations(await api.getDataLocations().catch(() => locations));
    } finally {
      setMovingData(false);
    }
  };

  const downloadJava = async () => {
    if (installingJava || javaMajors.length === 0) return;
    setInstallingJava(true);
    setJavaError(null);
    try {
      for (const major of [...javaMajors].sort((a, b) => a - b)) {
        await api.installJavaRuntime(major);
      }
      setJavas(await api.listJavas());
    } catch (error) {
      setJavaError(String(error));
    } finally {
      setInstallingJava(false);
    }
  };

  const finish = useCallback(async () => {
    if (!settings) return;
    setFinishing(true);
    const next: LauncherSettings = {
      ...settings,
      onboarded: true,
      ...(memory ? { min_memory_mb: memory.min, max_memory_mb: memory.max } : {}),
    };
    await api.updateSettings(next);
    useStore.setState({ settings: next });
  }, [settings, memory]);

  const meta = TITLES[step];
  const last = at >= steps.length - 1;

  return (
    <div className="relative flex h-full w-full flex-col overflow-hidden bg-base">
      <div
        aria-hidden
        className="pointer-events-none absolute -top-52 left-1/2 size-136 -translate-x-1/2 rounded-full opacity-[0.12] blur-3xl"
        style={{
          background: "radial-gradient(circle, var(--accent) 0%, transparent 65%)",
        }}
      />

      <div className="relative flex min-h-0 flex-1 items-center justify-center px-10 pt-9">
        <div className="w-full max-w-xl">
          <AnimatePresence mode="wait">
            <motion.div
              key={step}
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -12 }}
              transition={{ duration: 0.22, ease: "easeOut" }}
            >
              {step === "welcome" && (
                <motion.img
                  src="/logo.png"
                  alt=""
                  draggable={false}
                  initial={{ opacity: 0, scale: 0.86, y: 12 }}
                  animate={{ opacity: 1, scale: 1, y: 0 }}
                  transition={{ type: "spring", stiffness: 150, damping: 15 }}
                  className="mb-6 size-20 object-contain drop-shadow-[0_10px_36px_var(--accent-glow)]"
                />
              )}

              <motion.h1
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: step === "welcome" ? 0.12 : 0, duration: 0.35 }}
                className="font-display text-3xl font-bold tracking-tight text-content"
              >
                {meta.title}
              </motion.h1>
              <motion.p
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: step === "welcome" ? 0.2 : 0.05, duration: 0.35 }}
                className="mt-2 text-sm leading-relaxed text-content-muted"
              >
                {meta.blurb}
              </motion.p>

              <motion.div
                initial={{ opacity: 0, y: 12 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: step === "welcome" ? 0.3 : 0.1, duration: 0.35 }}
                className="mt-7 flex flex-col gap-2.5"
              >
                {step === "welcome" && (
                  <>
                    <Card
                      icon={<Boxes className="size-5" />}
                      title="Instances stay separate"
                      hint="Each instance keeps its own mods, worlds, settings, and Java setup."
                    />
                    <Card
                      icon={<Compass className="size-5" />}
                      title="Mods without the busywork"
                      hint="Browse Modrinth and CurseForge, install dependencies, and manage updates in one place."
                    />
                  </>
                )}

                {step === "storage" && (
                  <>
                    <div className="rounded-2xl border border-border-soft bg-surface-2/60 px-4 py-3.5">
                      <div className="flex items-start gap-3.5">
                        <span className="grid size-10 shrink-0 place-items-center rounded-xl bg-surface-3 text-(--accent)">
                          <HardDrive className="size-5" />
                        </span>
                        <div className="min-w-0 flex-1">
                          <div className="font-mono text-[11px] leading-relaxed break-all text-content">
                            {dataBase ?? "Reading the current folder"}
                          </div>
                          <div className="mt-1 text-xs text-content-faint">
                            {dataDisk
                              ? `${diskLabel(dataDisk)} · ${Math.round(dataDisk.free_mb / 1024)} GB free of ${Math.round(dataDisk.total_mb / 1024)} GB`
                              : "Free space is unknown on this drive"}
                          </div>
                        </div>
                        <button
                          onClick={() => void chooseDataFolder()}
                          disabled={movingData}
                          className="shrink-0 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs font-medium text-content transition-colors hover:bg-surface-3 disabled:opacity-50"
                        >
                          {movingData ? <Loader2 className="size-3.5 animate-spin" /> : "Another drive"}
                        </button>
                      </div>
                    </div>

                    {storageError && (
                      <div className="flex gap-2.5 rounded-2xl border border-danger/25 bg-danger/[0.07] px-4 py-3 text-xs text-danger">
                        <TriangleAlert className="mt-0.5 size-4 shrink-0" />
                        <span className="leading-relaxed">{storageError}</span>
                      </div>
                    )}

                    <p className="text-xs leading-relaxed text-content-faint">
                      Everything below goes onto the drive you pick. Or{" "}
                      <button
                        onClick={() => setLocationsOpen(true)}
                        className="font-medium text-content-muted underline decoration-border-soft underline-offset-2 transition-colors hover:text-content"
                      >
                        place each folder yourself
                      </button>
                      . Nothing is locked in either way, Settings, Storage moves any single folder
                      later, files and all.
                    </p>
                  </>
                )}

                {step === "import" && sources && (
                  <>
                    {sources.map((source) => (
                      <Card
                        key={source.root}
                        icon={<HardDriveDownload className="size-5" />}
                        title={source.label}
                        hint={`${source.instance_count} ${source.instance_count === 1 ? "instance" : "instances"} found`}
                      >
                        <button
                          onClick={() => setMigrateOpen(true)}
                          className="shrink-0 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs font-medium text-content transition-colors hover:bg-surface-3"
                        >
                          Import
                        </button>
                      </Card>
                    ))}
                    {instances.length > 0 && (
                      <div className="flex items-center gap-2 text-xs text-ok">
                        <Check className="size-3.5" />
                        {instances.length} {instances.length === 1 ? "instance" : "instances"} ready
                      </div>
                    )}
                  </>
                )}

                {step === "account" && (
                  <Card
                    icon={
                      account ? (
                        <PlayerHead uuid={account.id} name={account.name} size={28} />
                      ) : (
                        <UserCircle2 className="size-5" />
                      )
                    }
                    title={account ? account.name : "No account yet"}
                    hint={
                      account
                        ? "Signed in and ready to launch"
                        : "Opens the Microsoft device code page"
                    }
                  >
                    {!account && (
                      <button
                        onClick={() => void addAccount()}
                        className="shrink-0 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs font-medium text-content transition-colors hover:bg-surface-3"
                      >
                        Sign in
                      </button>
                    )}
                  </Card>
                )}

                {step === "java" && (
                  <div className="space-y-3">
                    <div className="flex gap-2.5 rounded-2xl border border-warn/25 bg-warn/[0.07] px-4 py-3.5 text-xs text-warn">
                      <TriangleAlert className="mt-0.5 size-4 shrink-0" />
                      <span className="leading-relaxed">
                        No Java runtime was found. Basalt can keep managed Eclipse Temurin
                        runtimes inside its own data folder, without changing system Java.
                      </span>
                    </div>

                    <div className="rounded-2xl border border-border-soft bg-surface-2/60 p-4">
                      <div className="mb-3 flex items-start gap-3">
                        <span className="grid size-10 shrink-0 place-items-center rounded-xl bg-surface-3 text-(--accent)">
                          <HardDriveDownload className="size-5" />
                        </span>
                        <div>
                          <div className="text-sm font-medium text-content">
                            Download runtimes for me
                          </div>
                          <div className="mt-0.5 text-xs text-content-faint">
                            Java 17 and 21 cover most current Minecraft versions. Pick others if
                            you play older or newer releases.
                          </div>
                        </div>
                      </div>

                      <div className="flex flex-wrap items-center gap-2">
                        {[8, 16, 17, 21, 25].map((major) => {
                          const selected = javaMajors.includes(major);
                          return (
                            <button
                              key={major}
                              onClick={() =>
                                setJavaMajors((current) =>
                                  selected
                                    ? current.filter((value) => value !== major)
                                    : [...current, major],
                                )
                              }
                              disabled={installingJava}
                              className={cn(
                                "rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors disabled:opacity-60",
                                selected
                                  ? "border-(--accent)/50 bg-(--accent-glow) text-(--accent-bright)"
                                  : "border-border bg-surface text-content-muted hover:text-content",
                              )}
                            >
                              Java {major}
                            </button>
                          );
                        })}
                        <button
                          onClick={() => void downloadJava()}
                          disabled={installingJava || javaMajors.length === 0}
                          className="ml-auto inline-flex items-center gap-2 rounded-lg bg-(--accent) px-3 py-1.5 text-xs font-semibold text-black transition-opacity hover:opacity-90 disabled:opacity-50"
                        >
                          {installingJava ? (
                            <Loader2 className="size-3.5 animate-spin" />
                          ) : (
                            <HardDriveDownload className="size-3.5" />
                          )}
                          {installingJava ? "Downloading" : "Download selected"}
                        </button>
                      </div>
                    </div>

                    <div className="flex items-center justify-between gap-3 px-1 text-xs">
                      <span className={javaError ? "text-danger" : "text-content-faint"}>
                        {javaError ?? "You can also install Java yourself and continue setup."}
                      </span>
                      <button
                        onClick={() => void openUrl("https://adoptium.net/temurin/releases/")}
                        className="shrink-0 font-medium text-content-muted transition-colors hover:text-content"
                      >
                        Install manually
                      </button>
                    </div>
                  </div>
                )}

                {step === "memory" && memory && (
                  <div className="rounded-2xl border border-border-soft bg-surface-2/60 px-4 py-4">
                    <div className="mb-3 flex items-center justify-between text-xs">
                      <span className="flex items-center gap-2 font-medium text-content">
                        <MemoryStick className="size-4 text-(--accent)" />
                        {(memory.max / 1024).toFixed(1)} GB ceiling
                      </span>
                      <span className="text-content-faint">
                        {stats ? `${(stats.total_memory_mb / 1024).toFixed(1)} GB installed` : ""}
                      </span>
                    </div>
                    <MemoryRange
                      min={memory.min}
                      max={memory.max}
                      ceiling={Math.max(4096, stats?.total_memory_mb ?? 16384)}
                      available={stats?.available_memory_mb}
                      onChange={(min, max) => setMemory({ min, max })}
                    />
                  </div>
                )}

                {step === "done" && (
                  <>
                    <Card
                      icon={<Boxes className="size-5" />}
                      title="Create an instance"
                      hint="Pick a version and a loader"
                    >
                      <button
                        onClick={() => setCreateOpen(true)}
                        className="shrink-0 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs font-medium text-content transition-colors hover:bg-surface-3"
                      >
                        Create
                      </button>
                    </Card>
                    <Card
                      icon={<Compass className="size-5" />}
                      title="Browse modpacks"
                      hint="Install a ready made pack in one step"
                    >
                      <button
                        onClick={async () => {
                          await finish();
                          openDiscover("modpacks");
                        }}
                        className="shrink-0 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-xs font-medium text-content transition-colors hover:bg-surface-3"
                      >
                        Browse
                      </button>
                    </Card>
                  </>
                )}

                {step === "java" && javas && javas.length > 0 && (
                  <Card
                    icon={<Coffee className="size-5" />}
                    title={`Java ${javas[0].major}`}
                    hint={javas[0].path}
                  />
                )}
              </motion.div>
            </motion.div>
          </AnimatePresence>
        </div>
      </div>

      <div className="relative flex shrink-0 items-center justify-between gap-4 px-8 py-6">
        <StepDots steps={steps} at={at} />

        <div className="flex items-center gap-2">
          {at > 0 && (
            <button
              onClick={() => setAt((v) => Math.max(0, v - 1))}
              disabled={installingJava}
              className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content"
            >
              Back
            </button>
          )}
          <button
            onClick={async () => {
              if (last) {
                await finish();
                setView("home");
                return;
              }
              setAt((v) => v + 1);
            }}
            disabled={finishing || installingJava}
            className="inline-flex items-center gap-2 rounded-lg px-5 py-2.5 text-sm font-semibold text-black shadow-lg shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:opacity-60"
          >
            {finishing && <Loader2 className="size-4 animate-spin" />}
            {last ? "Start playing" : "Continue"}
            {!last && <ArrowRight className="size-4" />}
          </button>
        </div>
      </div>

      <MigrateModal open={migrateOpen} onClose={() => setMigrateOpen(false)} />
      <DataLocationsModal
        open={locationsOpen}
        onClose={() => {
          setLocationsOpen(false);
          void api.getDataLocations().then(setLocations).catch(() => {});
        }}
      />
      <CreateInstanceModal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        onCreated={() => setCreateOpen(false)}
      />
      <SignInModal />
    </div>
  );
}
