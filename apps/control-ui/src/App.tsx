import { AppProvider } from "./state/AppContext";
import { AppShell } from "./components/shell/AppShell";
import { ServerProvider } from "./api/ServerContext";

export function App() {
  return <ServerProvider><AppProvider><AppShell /></AppProvider></ServerProvider>;
}
