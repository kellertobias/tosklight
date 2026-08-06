import { createContext, type ReactNode, useContext, useMemo } from "react";
import type { ControlMode } from "../../types";

export type ControlSurfacePolicy = {
	/** The pinned mode for this surface, or null to follow the desk-wide toggle. */
	mode: ControlMode | null;
	/** False while the surface carries only one of the two sections. */
	canToggle: boolean;
};

const ControlSurfaceModeContext = createContext<ControlSurfacePolicy | null>(
	null,
);

/**
 * Encoder placement decides which surface shows which section, so a surface that carries
 * only Playbacks (or only encoders) pins its mode and hides the Programmer/Playback toggle.
 */
export function ControlSurfaceModeProvider({
	mode,
	canToggle,
	children,
}: ControlSurfacePolicy & { children: ReactNode }) {
	const value = useMemo(() => ({ mode, canToggle }), [mode, canToggle]);
	return (
		<ControlSurfaceModeContext.Provider value={value}>
			{children}
		</ControlSurfaceModeContext.Provider>
	);
}

/** Null while the surface follows the desk-wide control mode. */
export function useControlSurfacePolicy() {
	return useContext(ControlSurfaceModeContext);
}
