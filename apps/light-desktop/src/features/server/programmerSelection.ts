import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createProgrammerSelectionActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	"undoProgrammer" | "setSelection" | "selectionGesture"
> {
	const {
		api,
		setError,
		selectedFixtures,
		setSelectedFixtures,
		selectedGroupId,
		setSelectedGroupId,
		refresh,
	} = model;
	return {
		undoProgrammer: async () => {
			try {
				await api.programming.undoProgrammer();
				await refresh();
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setSelection: async (fixtures) => {
			const previous = selectedFixtures;
			setSelectedFixtures(fixtures);
			setSelectedGroupId(null);
			try {
				await api.programming.setSelection(fixtures);
				setError(null);
			} catch (reason) {
				setSelectedFixtures(previous);
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		selectionGesture: async (source, remove = false) => {
			const previousFixtures = selectedFixtures;
			const previousGroup = selectedGroupId;
			try {
				const result = (await api.programming.selectionGesture(source, remove)) as {
					programmer?: {
						selected?: string[];
						selection_expression?: {
							type?: string;
							items?: Array<{ type?: string; group_id?: string }>;
						} | null;
					};
				};
				const programmer = result.programmer;
				setSelectedFixtures(programmer?.selected ?? []);
				const items =
					programmer?.selection_expression?.type === "sources"
						? (programmer.selection_expression.items ?? [])
						: [];
				const only = items.length === 1 ? items[0] : null;
				setSelectedGroupId(
					only?.type === "live_group" ? (only.group_id ?? null) : null,
				);
				setError(null);
			} catch (reason) {
				setSelectedFixtures(previousFixtures);
				setSelectedGroupId(previousGroup);
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
