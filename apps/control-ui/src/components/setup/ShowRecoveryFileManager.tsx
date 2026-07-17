import { useState } from "react";
import { useServer } from "../../api/ServerContext";
import { FileManager, type FileManagerSelection } from "../../windows/FileManagerWindow";

export function ShowRecoveryFileManager() {
  const server = useServer();
  const [message, setMessage] = useState("");
  const load = async (selection: FileManagerSelection[]) => {
    const selected = selection[0];
    if (!selected) return;
    setMessage(`Loading ${selected.entry.name} with safe blackout…`);
    const loaded = await server.openShowFile(selected.rootId, selected.entry.path, selected.entry.name);
    setMessage(loaded ? `${selected.entry.name} is now open.` : `Could not load ${selected.entry.name}.`);
  };

  return <section className="setup-show-file-manager" aria-label="Show file manager">
    <header>
      <h3>Show files</h3>
      <small>Select a root-confined .show file to load it through the normal safe-blackout transition.</small>
    </header>
    <div className="setup-show-file-manager-surface">
      <FileManager
        instanceId="setup-show-recovery"
        picker={{
          target: "files",
          allowedExtensions: ["show"],
          initialRootId: "shows",
          selectLabel: "Load selected show safely",
          hideCancel: true,
          onSelect: (selection) => void load(selection),
          onCancel: () => setMessage(""),
        }}
      />
    </div>
    {message && <p className="setup-show-load-status" role="status">{message}</p>}
  </section>;
}
