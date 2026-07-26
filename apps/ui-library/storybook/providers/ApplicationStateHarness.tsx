import { useEffect, useRef, type PropsWithChildren } from "react";
import { AppProvider, useApp } from "../../../light-desktop/src/state/AppContext";
import type { Action } from "../../../light-desktop/src/state/appReducer";

function ApplyStoryActions({ actions, children }: PropsWithChildren<{ actions: readonly Action[] }>) {
  const { dispatch } = useApp();
  const applied = useRef(false);
  useEffect(() => {
    if (applied.current) return;
    applied.current = true;
    actions.forEach(dispatch);
  }, [actions, dispatch]);
  return children;
}

export function ApplicationStateHarness({
  actions = [],
  children,
}: PropsWithChildren<{ actions?: readonly Action[] }>) {
  return <AppProvider>
    <ApplyStoryActions actions={actions}>{children}</ApplyStoryActions>
  </AppProvider>;
}
