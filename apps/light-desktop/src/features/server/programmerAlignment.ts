import type { ServerCapabilities } from "./capabilityContracts";
import type { ServerController } from "./model";

export function createProgrammerAlignmentActions(
	model: ServerController,
): Pick<ServerCapabilities, "alignSelection"> {
	const { api, setError } = model;
	return {
		alignSelection: async (attribute, mode) => {
			try {
				await api.programming.align(attribute, mode);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
