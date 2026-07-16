import { Button } from "../common";

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
  return (
    <div
      className="modal-backdrop"
      onPointerDown={(event) => event.target === event.currentTarget && onCancel()}
    >
      <section
        className="modal-card record-mode-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={`Record to ${target}`}
      >
        <Button className="modal-close" aria-label="Cancel recording" onClick={onCancel}>
          ×
        </Button>
        <h2>Record to {target}</h2>
        <p>Choose how the current programmer content is recorded into this existing target.</p>
        <div className="modal-actions three">
          <Button onClick={onCancel}>Cancel</Button>
          <Button onClick={() => onChoose("merge")}>Merge</Button>
          <Button className="primary" onClick={() => onChoose("overwrite")}>
            Overwrite
          </Button>
        </div>
      </section>
    </div>
  );
}
