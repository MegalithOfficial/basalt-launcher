import { useEffect } from "react";
import { AnimatePresence, motion } from "motion/react";

import { Sidebar } from "./components/Sidebar";
import { AccountsView } from "./views/AccountsView";
import { HomeView } from "./views/HomeView";
import { InstancesView } from "./views/InstancesView";
import { SettingsView } from "./views/SettingsView";
import { useStore } from "./store";
import type { View } from "./lib/types";

const VIEWS: Record<View, React.ComponentType> = {
  home: HomeView,
  instances: InstancesView,
  accounts: AccountsView,
  settings: SettingsView,
};

function App() {
  const view = useStore((s) => s.view);
  const ready = useStore((s) => s.ready);
  const error = useStore((s) => s.error);
  const init = useStore((s) => s.init);

  useEffect(() => {
    init();
  }, [init]);

  const Current = VIEWS[view];

  return (
    <div className="flex h-full w-full overflow-hidden bg-base text-content">
      <Sidebar />
      <main className="flex min-w-0 flex-1 flex-col">
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
  );
}

export default App;
