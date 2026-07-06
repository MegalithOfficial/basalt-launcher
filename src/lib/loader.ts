import type { Instance, LoaderKind } from "./types";

export const LOADERS: Array<{ id: LoaderKind; label: string }> = [
  { id: "fabric", label: "Fabric" },
  { id: "quilt", label: "Quilt" },
  { id: "neoforge", label: "NeoForge" },
  { id: "forge", label: "Forge" },
];

export function loaderLabel(instance: Instance): string {
  if (!instance.loader) return "Vanilla";
  return LOADERS.find((l) => l.id === instance.loader)?.label ?? instance.loader;
}

export function isInstanceInstalled(
  instance: Instance,
  installedVersions: string[],
): boolean {
  if (instance.loader) {
    return (
      !!instance.launch_version_id &&
      installedVersions.includes(instance.launch_version_id)
    );
  }
  return installedVersions.includes(instance.version_id);
}
