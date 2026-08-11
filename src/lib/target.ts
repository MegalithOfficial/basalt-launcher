export interface InstallTarget {
  id: string;
  name: string;
  version_id: string;
  loader: string | null;
  isServer: boolean;
}
