import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";
import { Button, ModalTitleBar } from "../common";

export function CommandChoiceModal() {
  const server = useServer();
  const choice = server.pendingCommandChoice;
  const [executing, setExecuting] = useState<string | null>(null);
  useEffect(() => setExecuting(null), [choice?.command]);
  if (!choice) return null;

  const select = async (command: string) => {
    setExecuting(command);
    const succeeded = await server.executeCommandLine(command);
    if (!succeeded) setExecuting(null);
  };

  const operation = choice.operation === "copy" ? "Copy" : "Move";
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
          <Button disabled={executing !== null} onClick={server.cancelCommandChoice}>{choice.cancel_label}</Button>
        </div>
      </section>
    </div>,
    document.body,
  );
}
