import { useEffect, useRef } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";

export function LayoutPersistence() {
  const server = useServer();
  const { state, dispatch } = useApp();
  const appliedRevision = useRef<number | null>(null);
  const hydrated = useRef(false);

  useEffect(() => {
    if (!server.deskLayout) { if (server.bootstrap?.active_show) hydrated.current = true; return; }
    if (appliedRevision.current === server.deskLayout.revision) return;
    appliedRevision.current = server.deskLayout.revision;
    hydrated.current = true;
    dispatch({
      type: "HYDRATE_LAYOUT",
      desks: server.deskLayout.body.desks,
      activeDeskId: server.deskLayout.body.activeDeskId,
    });
  }, [server.deskLayout, server.bootstrap?.active_show, dispatch]);

  useEffect(() => {
    if (!hydrated.current || !server.bootstrap?.active_show) return;
    const timer = window.setTimeout(() => void server.saveDeskLayout({ desks: state.desks, activeDeskId: state.activeDeskId }), 600);
    return () => window.clearTimeout(timer);
  }, [state.desks, state.activeDeskId, server]);

  return null;
}
