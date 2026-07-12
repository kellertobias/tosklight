import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";

const keys = ["7", "8", "9", "AT", "4", "5", "6", "FULL", "1", "2", "3", "THRU", "GROUPS", "SET", "STORE", "CLEAR", ".", "0", "←", "ENTER"];
export function NumericPad() {
  const server = useServer();
  const { state, dispatch } = useApp();
  const press = (key: string) => {
    if (key === "←") return server.setCommandLine(server.commandLine.slice(0, -1));
    if (key === "CLEAR") { server.setCommandLine(""); if (state.preload !== "idle") void server.preloadAction("clear"); return; }
    if (key === "STORE" && state.preload !== "idle") return dispatch({ type: "SET_MODAL", modal: "preloadStoreOpen", value: true });
    if (key === "ENTER") return void server.executeCommandLine();
    const token = ["AT", "FULL", "THRU", "GROUPS", "SET", "STORE"].includes(key) ? ` ${key} ` : key;
    server.setCommandLine(`${server.commandLine}${token}`.replace(/\s+/g, " ").trimStart());
  };
  return <div className="numeric-pad">{keys.map((key) => <button onClick={() => press(key)} className={["AT", "FULL", "THRU", "GROUPS", "SET", "STORE", "CLEAR"].includes(key) ? "action" : key === "ENTER" ? "enter" : ""} key={key}>{key}</button>)}</div>;
}
