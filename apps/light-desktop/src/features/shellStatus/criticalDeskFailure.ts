import type { DeskStateDiagnostic } from "../deskState/deskStateDiagnostics";

export interface CriticalDeskFailure {
	capability: "show" | "dmx_output";
	message: string;
	action: string;
}

/** Request rejection and historical send counters do not prove capability loss. */
export function criticalDeskFailure(
	activeShowError: string | null,
	diagnostics: readonly DeskStateDiagnostic[],
): CriticalDeskFailure | null {
	if (activeShowError)
		return {
			capability: "show",
			message: activeShowError,
			action:
				"Use Show recovery to load a working show. The damaged file is preserved.",
		};
	const output = diagnostics.find(
		(entry) => entry.capabilityLoss === "dmx_output",
	);
	return output
		? {
				capability: "dmx_output",
				message: output.summary,
				action: output.action,
			}
		: null;
}
