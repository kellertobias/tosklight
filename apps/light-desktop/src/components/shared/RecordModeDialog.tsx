import { Button, ModalPortal, ModalTitleBar } from "@tosklight/ui";

export type RecordMode = "merge" | "overwrite";

export function RecordModeDialog({
  target,
  onChoose,
  onCancel,
}: {
  target: string;
  onChoose: (mode: RecordMode) => void;
  onCancel: () => void;
}) {
  return <ModalPortal onClose={onCancel}>
    <div
      className="modal-backdrop"
      onPointerDown={(event) => event.target === event.currentTarget && onCancel()}
    >
      <section
        className="modal-card record-mode-dialog workflow-theme record-workflow"
        role="dialog"
        aria-modal="true"
        aria-label={`Record to ${target}`}
      >
		<ModalTitleBar title={<><span className="workflow-badge">RECORD</span> Record to {target}</>} closeLabel="Cancel recording" onClose={onCancel}/>
        <p>Choose how the current programmer content is recorded into this existing target.</p>
        <div className="modal-actions three">
          <Button onClick={onCancel}>Cancel</Button>
          <Button className="workflow-choice" onClick={() => onChoose("merge")}>Merge</Button>
          <Button className="danger" onClick={() => onChoose("overwrite")}>
            Overwrite
          </Button>
        </div>
      </section>
    </div>
  </ModalPortal>;
}
