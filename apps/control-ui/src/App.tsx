import { AppProvider } from "./state/AppContext";
import { AppShell } from "./components/shell/AppShell";
import { QuitConfirmOverlay } from "./components/modals/QuitConfirmOverlay";
import { ServerProvider } from "./api/ServerContext";

export function App() {
  return <ServerProvider><AppProvider><AppShell /><QuitConfirmOverlay /></AppProvider></ServerProvider>;
}
