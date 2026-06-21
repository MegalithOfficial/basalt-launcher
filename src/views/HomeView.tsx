import { motion } from "motion/react";
import { Boxes, Play, Sparkles } from "lucide-react";

import { Button, PageHeader } from "../components/ui";
import { useStore } from "../store";

export function HomeView() {
  const setView = useStore((s) => s.setView);
  const instances = useStore((s) => s.instances);

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader title="Play" subtitle="Launch Minecraft and keep an eye on it." />

      <div className="flex-1 overflow-y-auto p-8">
        <motion.div
          initial={{ opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.35, ease: "easeOut" }}
          className="relative overflow-hidden rounded-2xl border border-border bg-gradient-to-br from-surface-2 to-surface p-8"
        >
          <div className="pointer-events-none absolute -right-16 -top-16 size-56 rounded-full bg-lava/10 blur-3xl" />
          <div className="relative max-w-lg">
            <div className="mb-3 inline-flex items-center gap-1.5 rounded-full border border-border bg-surface-3/60 px-2.5 py-1 text-[11px] font-medium text-ember">
              <Sparkles className="size-3" />
              Welcome to Basalt
            </div>
            <h2 className="font-display text-3xl font-bold tracking-tight text-content">
              Forge your launch.
            </h2>
            <p className="mt-2 text-sm leading-relaxed text-content-muted">
              {instances.length === 0
                ? "Create your first instance to pick a version and start playing. Mods and modpacks land later."
                : `You have ${instances.length} instance${instances.length === 1 ? "" : "s"} ready to play.`}
            </p>
            <div className="mt-6 flex gap-3">
              <Button onClick={() => setView("instances")}>
                <Boxes className="size-4" />
                {instances.length === 0 ? "Create instance" : "Browse instances"}
              </Button>
              <Button variant="ghost" onClick={() => setView("accounts")}>
                <Play className="size-4" />
                Add account
              </Button>
            </div>
          </div>
        </motion.div>
      </div>
    </div>
  );
}
