import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Button, ModalTitleBar } from "../common";
import { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";

export function CommandChoiceModal() {
  // Choice visibility remains tied to the explicit execute response. The v2 text
  // projection can infer an ambiguous Cue transfer before the operator presses ENT.
  const commandLine = useCommandLineSurface({ observeCommand: false });
  const choice = commandLine.pendingChoice;
  const [executing, setExecuting] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState<string | null>(null);
  useEffect(() => {
    setExecuting(null);
    if (!choice) setDismissed(null);
  }, [choice]);
  if (!choice || choice.command === dismissed) return null;

  const select = async (command: string) => {
    setExecuting(command);
    const succeeded = await commandLine.execute(command);
    if (!succeeded) setExecuting(null);
  };

  const operation = choice.operation === "copy" ? "Copy" : "Move";
  const cancelLabel = "cancelLabel" in choice ? choice.cancelLabel : choice.cancel_label;
  const cancel = () => {
    setDismissed(choice.command);
    void commandLine.cancelChoice();
  };
  return createPortal(
    <div className="stacked-modal-layer command-choice-layer">
      <section className="nested-modal command-choice-modal" role="dialog" aria-modal="true" aria-label={`Cue ${operation} choice`}>
        <ModalTitleBar title={`Cue ${operation}`} />
        <p>Choose whether to transfer only the stored Cue delta or its complete tracked status.</p>
        <div className="command-choice-actions">
          {choice.options.map((option) => <Button
            key={option.id}
            variant="primary"
            loading={executing === option.command}
            disabled={executing !== null}
            onClick={() => void select(option.command)}
          >{option.label}</Button>)}
          <Button disabled={executing !== null} onClick={cancel}>{cancelLabel}</Button>
        </div>
      </section>
    </div>,
    document.body,
  );
}
