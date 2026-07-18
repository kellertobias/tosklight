import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createGroupSelectionActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	"applyGroup" | "selectGroup" | "selectionMacro" | "alignSelection"
> {
	const { client, setError, setSelectedFixtures, setSelectedGroupId } = model;
	return {
		applyGroup: async (id) => {
			try {
				const result = (await client.selectGroup(id)) as {
					programmer?: { selected?: string[] };
				};
				setSelectedFixtures(result.programmer?.selected ?? []);
				setSelectedGroupId(id);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		selectGroup: async (id, frozen = false, rule = { type: "all" }) => {
			try {
				const result = (await client.selectGroup(id, frozen, rule)) as {
					programmer?: { selected?: string[] };
				};
				const selected = result.programmer?.selected ?? [];
				setSelectedFixtures(selected);
				setSelectedGroupId(frozen ? null : id);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		selectionMacro: async (rule) => {
			try {
				const result = (await client.selectionMacro(rule)) as {
					programmer?: { selected?: string[] };
				};
				setSelectedFixtures(result.programmer?.selected ?? []);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		alignSelection: async (attribute, mode) => {
			try {
				await client.align(attribute, mode);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
