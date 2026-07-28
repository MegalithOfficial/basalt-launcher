import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion } from "motion/react";

import { accentVars } from "./lib/accent";
import { cn } from "./lib/cn";
import { Sidebar } from "./components/Sidebar";
import { RecoveryBanner } from "./components/RecoveryBanner";
import { TitleBar } from "./components/TitleBar";
import { Toaster } from "sonner";
import { AccountsView } from "./views/AccountsView";
import { ConsoleView } from "./views/ConsoleView";
import { HomeView } from "./views/HomeView";
import { InstanceView } from "./views/InstanceView";
import { InstancesView } from "./views/InstancesView";
import { DiscoverView } from "./views/DiscoverView";
import { LogsView } from "./views/LogsView";
import { ProjectView } from "./views/ProjectView";
import { SettingsView } from "./views/SettingsView";
import { useStore } from "./store";
import type { View } from "./lib/types";

const VIEWS: Record<View, React.ComponentType> = {
  home: HomeView,
  instances: InstancesView,
  accounts: AccountsView,
  settings: SettingsView,
  console: ConsoleView,
  instance: InstanceView,
  discover: DiscoverView,
  project: ProjectView,
  logs: LogsView,
};

function App() {
  const view = useStore((s) => s.view);
  const ready = useStore((s) => s.ready);
  const error = useStore((s) => s.error);
  const init = useStore((s) => s.init);
  const accent = useStore((s) =>
    s.selectedInstanceId ? (s.media[s.selectedInstanceId]?.accent ?? null) : null,
  );

  const [maximized, setMaximized] = useState(false);

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

  const Current = VIEWS[view];
  const immersive = view === "instance" || view === "project";

  return (
    <div
      className={cn(
        "flex h-full w-full overflow-hidden bg-base text-content",
        !maximized && "rounded-xl border border-border-soft",
      )}
      style={accentVars(accent)}
    >
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
      <Sidebar />
      <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
        <TitleBar immersive={immersive} />
        <main className="flex min-h-0 min-w-0 flex-1 flex-col">
        <RecoveryBanner />
        {!ready ? (
          <div className="grid flex-1 place-items-center text-sm text-content-muted">
            Loading…
          </div>
        ) : error ? (
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
              <Current />
            </motion.div>
          </AnimatePresence>
        )}
        </main>
      </div>
    </div>
  );
}

export default App;
