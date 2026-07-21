import type { ConnectionStatus } from "../../api/types";
import { useConnectionStatus } from "../../features/shellStatus/ShellStatusState";

export type ShowIndicator = {
  className: "show-status-connected" | "show-status-disconnected";
  label: string;
  detail: string;
  connected: boolean;
};

export function getShowIndicator(status: ConnectionStatus): ShowIndicator {
  if (status !== "connected") {
    return {
      className: "show-status-disconnected",
      connected: false,
      label: status === "connecting" ? "Server connecting" : "Server disconnected",
      detail: status === "connecting"
        ? "The desk is trying to connect to the server."
        : "Show changes cannot be confirmed until the server reconnects.",
    };
  }
  return {
    className: "show-status-connected",
    connected: true,
    label: "Show active",
    detail: "Changes are saved automatically as they are made.",
  };
}

/** The show indicator for the current connection status. */
export function useShowIndicator(): ShowIndicator {
  return getShowIndicator(useConnectionStatus());
}
