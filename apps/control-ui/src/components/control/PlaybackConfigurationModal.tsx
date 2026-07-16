import { createPortal } from "react-dom";
import type { PlaybackDefinition } from "../../api/types";
import { Button } from "../common";
import { ModalTitleBar } from "../common/ModalTitleBar";

export function PlaybackConfigurationModal({ playback, page, slot, onUnassign, onClose }: {
  playback: PlaybackDefinition;
  page: number;
  slot: number;
  onUnassign: () => Promise<boolean>;
  onClose: () => void;
}) {
  return createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="nested-modal playback-configuration-modal" role="dialog" aria-modal="true" aria-label={`Configure playback ${page}.${slot}`}>
      <ModalTitleBar
        title={`Playback ${page}.${slot} · ${playback.name}`}
        actions={<Button variant="danger" onClick={() => void onUnassign().then((ok) => ok && onClose())}>Unassign Playback</Button>}
        onClose={onClose}
        closeLabel="Close playback configuration"
      />
      <div className="playback-configuration-body" />
    </section>
  </div>, document.body);
}
