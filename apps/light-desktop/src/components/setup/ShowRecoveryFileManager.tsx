import { useApp } from "../../state/AppContext";
import { Button } from "../common";

export function ShowRecoveryFileManager({ onOpenFixtureLibrary }: { onOpenFixtureLibrary: () => void }) {
  const { dispatch } = useApp();
  return <section className="setup-show-file-manager" aria-label="Show file manager">
    <header>
      <h3>Show files</h3>
      <small>Open the desk file tools or manage the desk-wide fixture library.</small>
    </header>
    <div className="setup-show-file-actions">
      <Button onClick={() => dispatch({ type: "OPEN_BUILTIN", kind: "file_manager" })}>Open File Manager</Button>
      <Button onClick={onOpenFixtureLibrary}>Open Fixture Library</Button>
    </div>
  </section>;
}
