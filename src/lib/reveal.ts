import { toast } from "sonner";

import { api } from "./api";

export function openFolder(path: string) {
  void api.openFolder(path).catch((error) =>
    toast.error("Could not open the folder", { description: String(error) }),
  );
}

export function openFile(path: string) {
  void api.openFile(path).catch((error) =>
    toast.error("Could not open the file", { description: String(error) }),
  );
}
