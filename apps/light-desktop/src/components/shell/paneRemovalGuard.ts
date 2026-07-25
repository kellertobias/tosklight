export type PaneRemovalGuard = () => string | null;

const paneRemovalGuards = new Map<string, Set<PaneRemovalGuard>>();

/**
 * Register transient state that would be lost if a configurable pane were
 * removed. The state itself deliberately stays in the owning window; the
 * shell only needs a synchronous explanation before it mutates the layout.
 */
export function registerPaneRemovalGuard(paneId: string, guard: PaneRemovalGuard) {
  const guards = paneRemovalGuards.get(paneId) ?? new Set<PaneRemovalGuard>();
  guards.add(guard);
  paneRemovalGuards.set(paneId, guards);
  return () => {
    guards.delete(guard);
    if (!guards.size) paneRemovalGuards.delete(paneId);
  };
}

export function requestPaneRemoval(paneId: string, confirmRemoval: (message: string) => boolean = window.confirm) {
  const reasons = [...(paneRemovalGuards.get(paneId) ?? [])]
    .map((guard) => guard())
    .filter((reason): reason is string => Boolean(reason));
  if (!reasons.length) return true;
  return confirmRemoval(`${[...new Set(reasons)].join("\n\n")}\n\nRemove this pane and discard those changes?`);
}
