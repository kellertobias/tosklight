import { createContext, type PropsWithChildren, useContext } from "react";
import type {
	VisualizerView,
	VisualizerViewPatch,
	VisualizerViewSnapshot,
} from "../../api/client/visualizerView";

/**
 * What the desk tells its connected visualizers to look at.
 *
 * The renderer takes its scene from the API and its live values from the lighting network. The
 * one thing it cannot work out for itself is which way the operator wants to be looking, so the
 * desk keeps that and publishes it. Addressed by renderer target, so a desk driving two windows
 * can move one of them.
 */
export interface VisualizerViewActions {
	snapshot(): Promise<VisualizerViewSnapshot>;
	update(target: string, patch: VisualizerViewPatch): Promise<VisualizerView>;
	onConnectionChanged(listener: (connected: boolean) => void): () => unknown;
	/**
	 * Whether an Architect is following the active show, kept in step with Control. Absent where
	 * no desk is attached, which reads as no Architect.
	 */
	architectSync?(): Promise<boolean>;
	onArchitectSyncChanged?(listener: (active: boolean) => void): () => unknown;
}

const VisualizerViewContext = createContext<VisualizerViewActions | null>(null);

export function VisualizerViewProvider({
	children,
	actions,
}: PropsWithChildren<{ actions: VisualizerViewActions }>) {
	return (
		<VisualizerViewContext.Provider value={actions}>
			{children}
		</VisualizerViewContext.Provider>
	);
}

/** The visualizer-view actions, or null outside a mounted desk boundary. */
export function useVisualizerViewActions(): VisualizerViewActions | null {
	return useContext(VisualizerViewContext);
}
