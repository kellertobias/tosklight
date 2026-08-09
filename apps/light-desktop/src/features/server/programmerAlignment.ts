import type { ServerCapabilities } from "./capabilityContracts";
import type { ServerController } from "./model";

export function createProgrammerAlignmentActions(
	model: ServerController,
): Pick<ServerCapabilities, "alignSelection"> {
	const { api, setError } = model;
	return {
		alignSelection: async (mode) => {
			try {
				await api.programming.align(mode);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
	};
}
