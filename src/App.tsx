import { lazy, Suspense, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion } from "motion/react";

import { applyTheme, themeVars } from "./lib/accent";
import { cn } from "./lib/cn";
import { Onboarding } from "./components/onboarding/Onboarding";
import { Sidebar } from "./components/Sidebar";
import { RecoveryBanner } from "./components/RecoveryBanner";
import { TitleBar } from "./components/TitleBar";
import { WindowFrame } from "./components/WindowFrame";
import { UpdateNotifications } from "./components/UpdateNotifications";
import { ContentInstallerProvider } from "./components/CurseForgeDownloadModal";
import { Toaster } from "sonner";
import { AccountsView } from "./views/AccountsView";
import { HomeView } from "./views/HomeView";
import { InstanceView } from "./views/InstanceView";
import { InstancesView } from "./views/InstancesView";
import { ServerView } from "./views/ServerView";
import { ServersView } from "./views/ServersView";
import { DiscoverView } from "./views/DiscoverView";
import { LogsView } from "./views/LogsView";
import { ProjectView } from "./views/ProjectView";
import { SettingsView } from "./views/SettingsView";
import { useStore } from "./store";
import type { View } from "./lib/types";

const StatsView = lazy(() =>
  import("./views/StatsView").then((module) => ({ default: module.StatsView })),
);

const VIEWS: Record<View, React.ComponentType> = {
  home: HomeView,
  instances: InstancesView,
  accounts: AccountsView,
  settings: SettingsView,
  instance: InstanceView,
  servers: ServersView,
  server: ServerView,
  discover: DiscoverView,
  project: ProjectView,
  stats: StatsView,
  logs: LogsView,
};

function App() {
  const view = useStore((s) => s.view);
  const ready = useStore((s) => s.ready);
  const error = useStore((s) => s.error);
  const init = useStore((s) => s.init);
  const banner = useStore((s) =>
    s.selectedInstanceId ? (s.media[s.selectedInstanceId]?.accent ?? null) : null,
  );
  const settings = useStore((s) => s.settings);

  const [maximized, setMaximized] = useState(false);
  const onboarding = useStore((s) => s.ready && s.settings?.onboarded === false);

  useEffect(() => {
    init();
  }, [init]);

  useEffect(() => {
    const win = getCurrentWindow();
    const sync = () => win.isMaximized().then(setMaximized);
    sync();
    const unlisten = win.onResized(sync);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    applyTheme(themeVars(settings, onboarding ? null : banner));
  }, [settings, banner, onboarding]);


  const Current = VIEWS[view];
  const immersive = view === "instance" || view === "project";

  return (
    <ContentInstallerProvider>
    <div
      className={cn(
        "flex h-full w-full overflow-hidden bg-void text-content",
        !maximized && "rounded-xl border border-border-soft",
      )}
    >
      <WindowFrame enabled={!maximized} />
      <Toaster
        theme="dark"
        position="bottom-left"
        offset={16}
        gap={8}
        visibleToasts={4}
        toastOptions={{
          classNames: {
            toast:
              "!bg-surface !border !border-border !text-content !rounded-xl !shadow-2xl !font-sans",
            title: "!text-[13px] !font-medium !text-content",
            description: "!text-[11px] !text-content-muted",
            success: "!border-ok/40",
            error: "!border-danger/40",
            closeButton: "!bg-surface-2 !border-border !text-content-faint",
          },
        }}
      />
      <UpdateNotifications />
      {!ready ? (
        <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
          <TitleBar />
          <div className="grid flex-1 place-items-center pt-9">
            <img
              src="/logo.png"
              alt=""
              draggable={false}
              className="size-12 animate-pulse object-contain opacity-60"
            />
          </div>
        </div>
      ) : onboarding ? (
        <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
          <TitleBar />
          <Onboarding />
        </div>
      ) : (
        <>
      <Sidebar />
      <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
        <TitleBar immersive={immersive} />
        <main className="flex min-h-0 min-w-0 flex-1 flex-col pt-9">
        <RecoveryBanner />
        {error ? (
          <div className="grid flex-1 place-items-center px-8 text-center">
            <div>
              <div className="font-display text-lg font-semibold text-danger">
                Failed to start
              </div>
              <p className="mt-1 max-w-md text-sm text-content-muted">{error}</p>
            </div>
          </div>
        ) : (
          <AnimatePresence mode="wait">
            <motion.div
              key={view}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.15 }}
              className="flex min-h-0 flex-1 flex-col"
            >
              <Suspense fallback={<div className="flex-1" />}>
                <Current />
              </Suspense>
            </motion.div>
          </AnimatePresence>
        )}
        </main>
      </div>
        </>
      )}
    </div>
    </ContentInstallerProvider>
  );
}

export default App;
