import type { ServerCapabilities } from "./capabilityContracts";
import type { ServerController } from "./model";

export function createProgrammerAlignmentActions(
	model: ServerController,
): Pick<ServerCapabilities, "alignSelection"> {
	const { api, setError } = model;
	return {
		alignSelection: async (mode) => {
			const store = model.programmingInteractionStore;
			const scope = store.captureScope();
			let resulting: Awaited<ReturnType<typeof api.programming.align>>;
			try {
				resulting = await api.programming.align(mode);
				store.installAlignment(resulting, scope);
				setError(null);
			} catch (reason) {
				// Refusals and genuine desk failures keep the actionable error treatment.
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
			return resulting.mode;
		},
	};
}
