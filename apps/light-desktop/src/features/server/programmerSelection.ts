import { formatErrorDetails } from "@tosklight/ui";
import type { ServerCapabilities } from "./capabilityContracts";
import type { ServerController } from "./model";

export function createProgrammerSelectionActions(
	model: ServerController,
): Pick<ServerCapabilities, "undoProgrammer" | "toggleFixtureFreeze"> {
	const { api, setError } = model;
	return {
		undoProgrammer: async () => {
			try {
				await api.programming.undoProgrammer();
				setError(null);
			} catch (reason) {
				setError(formatErrorDetails(reason));
			}
		},
		toggleFixtureFreeze: async () => {
			try {
				await api.programming.toggleFixtureFreeze();
				setError(null);
			} catch (reason) {
				setError(formatErrorDetails(reason));
			}
		},
	};
}
