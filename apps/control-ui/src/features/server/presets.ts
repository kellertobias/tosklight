import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createPresetActions(
	model: ServerController,
): Pick<ServerContextValue, "applyPreset"> {
	const { client, setError, setSelectedFixtures } = model;
	return {
		applyPreset: async (address) => {
			try {
				const result = (await client.applyPreset(address)) as
					| { programmer?: { selected?: string[] } }
					| undefined;
				if (result?.programmer?.selected)
					setSelectedFixtures(result.programmer.selected);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
