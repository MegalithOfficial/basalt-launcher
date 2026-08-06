import { HardDrive } from "lucide-react";

import { Modal, ModalBody, ModalFooter, ModalHeader } from "../Modal";
import { DataLocations } from "./DataLocations";

export function DataLocationsModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Modal open={open} onClose={onClose} size="full" labelledBy="data-locations-title">
      <ModalHeader
        id="data-locations-title"
        title="Place each folder"
        subtitle="Point any of these at another drive. Basalt moves what is already there."
        icon={<HardDrive className="size-5 text-(--accent)" />}
        onClose={onClose}
      />
      <ModalBody>
        <DataLocations heading={false} />
      </ModalBody>
      <ModalFooter>
        <button
          onClick={onClose}
          className="rounded-lg border border-border bg-surface-2 px-3.5 py-2 text-xs font-medium text-content transition-colors hover:bg-surface-3"
        >
          Done
        </button>
      </ModalFooter>
    </Modal>
  );
}
