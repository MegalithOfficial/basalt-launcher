import { createContext } from "react";

import type {
  ContentKind,
  InstalledItem,
  Instance,
  SearchProvider,
} from "./types";

export interface ContentInstallOptions {
  provider: SearchProvider;
  projectId: string;
  instanceId: string;
  kind: ContentKind;
  gameVersion: string;
  loader: string | null;
  versionId?: string | null;
  title?: string;
  iconUrl?: string | null;
}

export interface ContentInstaller {
  installContent: (options: ContentInstallOptions) => Promise<InstalledItem[] | null>;
  installPack: (
    provider: SearchProvider,
    projectId: string,
    versionId: string,
    title?: string,
    iconUrl?: string | null,
  ) => Promise<Instance | null>;
  installLatestPack: (
    provider: SearchProvider,
    projectId: string,
    title?: string,
    iconUrl?: string | null,
  ) => Promise<Instance | null>;
  installingVersionId: string | null;
}

const unavailable = async (): Promise<never> => {
  throw new Error("The content installer is reloading. Try again.");
};

export const ContentInstallerContext = createContext<ContentInstaller>({
  installContent: unavailable,
  installPack: unavailable,
  installLatestPack: unavailable,
  installingVersionId: null,
});
