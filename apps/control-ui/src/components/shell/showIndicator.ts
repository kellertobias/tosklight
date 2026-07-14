import type { ConnectionStatus } from "../../api/types";

export type ShowIndicator = {
  className: "show-status-connected" | "show-status-disconnected";
  label: string;
  detail: string;
};

export function getShowIndicator(status: ConnectionStatus): ShowIndicator {
  if (status !== "connected") {
    return {
      className: "show-status-disconnected",
      label: status === "connecting" ? "Server connecting" : "Server disconnected",
      detail: status === "connecting"
        ? "The desk is trying to connect to the server."
        : "Show changes cannot be confirmed until the server reconnects.",
    };
  }
  return {
    className: "show-status-connected",
    label: "Show active",
    detail: "Changes are saved automatically as they are made.",
  };
}
