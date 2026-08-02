import { useMemo } from "react";

import type { Task } from "./types";
import { useStore } from "../store";

const NO_TASKS: Task[] = [];

export function isActive(task: Task): boolean {
  return task.state === "running";
}

function ordered(tasks: Record<string, Task>, order: string[]): Task[] {
  return order.map((id) => tasks[id]).filter(Boolean);
}

export function useTasks(): Task[] {
  const tasks = useStore((s) => s.tasks);
  const order = useStore((s) => s.taskOrder);
  return useMemo(() => ordered(tasks, order), [tasks, order]);
}

export function useActiveTasks(): Task[] {
  const all = useTasks();
  return useMemo(() => all.filter(isActive), [all]);
}

export function useFinishedTasks(): Task[] {
  const all = useTasks();
  return useMemo(() => all.filter((t) => !isActive(t)).reverse(), [all]);
}

export function useInstanceTask(instanceId: string | null | undefined): Task | null {
  const active = useActiveTasks();
  return useMemo(
    () => (instanceId ? (active.find((t) => t.instance_id === instanceId) ?? null) : null),
    [active, instanceId],
  );
}

export function useProjectTask(projectId: string | null | undefined): Task | null {
  const active = useActiveTasks();
  return useMemo(
    () => (projectId ? (active.find((t) => t.project_id === projectId) ?? null) : null),
    [active, projectId],
  );
}

export function useActiveTasksByInstance(): Map<string, Task> {
  const active = useActiveTasks();
  return useMemo(() => {
    const map = new Map<string, Task>();
    for (const task of active) {
      if (task.instance_id && !map.has(task.instance_id)) map.set(task.instance_id, task);
    }
    return map;
  }, [active]);
}

export function useActiveTasksByProject(): Map<string, Task> {
  const active = useActiveTasks();
  return useMemo(() => {
    const map = new Map<string, Task>();
    for (const task of active) {
      if (task.project_id && !map.has(task.project_id)) map.set(task.project_id, task);
    }
    return map;
  }, [active]);
}

export function useActiveProjectIds(): Set<string> {
  const active = useActiveTasks();
  return useMemo(
    () => new Set(active.map((t) => t.project_id).filter((id): id is string => !!id)),
    [active],
  );
}

export function useTasksForInstance(instanceId: string | null | undefined): Task[] {
  const all = useTasks();
  return useMemo(() => {
    if (!instanceId) return NO_TASKS;
    const matched = all.filter((t) => t.instance_id === instanceId);
    return matched.length > 0 ? matched : NO_TASKS;
  }, [all, instanceId]);
}

export function taskFraction(task: Task): number | null {
  if (task.total_bytes > 0) return task.downloaded_bytes / task.total_bytes;
  if (task.total > 0) return task.completed / task.total;
  return null;
}

export function instanceTaskLabel(task: Task): string {
  if (task.kind === "instance_repair") return "Repairing";
  if (task.kind === "instance_duplicate") return "Duplicating";
  if (task.kind === "modpack_upgrade") return "Upgrading modpack";
  if (task.kind === "snapshot_create") return "Creating snapshot";
  if (task.kind === "snapshot_restore") return "Restoring snapshot";
  return task.stage.replace(/-/g, " ");
}

export function aggregateFraction(tasks: Task[]): number | null {
  const bytes = tasks.filter((t) => t.total_bytes > 0);
  if (bytes.length > 0) {
    const done = bytes.reduce((n, t) => n + t.downloaded_bytes, 0);
    const total = bytes.reduce((n, t) => n + t.total_bytes, 0);
    return total > 0 ? done / total : null;
  }
  const counted = tasks.filter((t) => t.total > 0);
  if (counted.length === 0) return null;
  const done = counted.reduce((n, t) => n + t.completed, 0);
  const total = counted.reduce((n, t) => n + t.total, 0);
  return total > 0 ? done / total : null;
}
