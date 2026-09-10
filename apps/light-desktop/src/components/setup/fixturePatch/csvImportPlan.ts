import { useMemo } from "react";
import type { PatchController } from "./controller";
import {
	type CsvConflictPolicy,
	type CsvImportRowPlan,
	type CsvTypeMapping,
	csvImportCandidates,
	csvSourceTypes,
	planCsvImport,
} from "./csvImport";
import type { useCsvImportSource } from "./csvImportSource";

/** Derives the fixture types, per-row import plan, and target layer for the current choices. */
export function useCsvImportPlan({
	controller,
	source,
	mappings,
	conflictPolicy,
}: {
	controller: PatchController;
	source: ReturnType<typeof useCsvImportSource>;
	mappings: Readonly<Record<string, CsvTypeMapping>>;
	conflictPolicy: CsvConflictPolicy;
}) {
	const definitions = controller.data.availableDefinitions;
	const sourceTypes = useMemo(
		() => csvSourceTypes(source.dataRows, source.assignments, definitions),
		[definitions, source.assignments, source.dataRows],
	);
	const unresolved = sourceTypes.filter(
		(item) => !(mappings[item.key] ?? item.exactMatch),
	);
	const plans = useMemo(
		() =>
			planCsvImport({
				rows: source.dataRows,
				lines: source.rowNumbers,
				assignments: source.assignments,
				sourceTypes,
				mappings,
				existing: controller.data.all,
				positionUnit: source.positionUnit,
				conflictPolicy,
			}),
		[
			conflictPolicy,
			controller.data.all,
			mappings,
			source.assignments,
			source.dataRows,
			source.positionUnit,
			source.rowNumbers,
			sourceTypes,
		],
	);
	const importable = plans.filter(
		(plan) => plan.status === "ready" || plan.status === "unpatched",
	);
	const layerId =
		controller.ui.activeLayer === "all" ? "default" : controller.ui.activeLayer;
	const layerName =
		controller.data.layers.find((layer) => layer.id === layerId)?.name ??
		layerId;
	return { sourceTypes, unresolved, plans, importable, layerId, layerName };
}

/**
 * Adds every importable row in one Patch mutation, which the server validates as a whole. Closes
 * the dialog on success and returns an operator-facing error otherwise.
 */
export async function runCsvImport(
	controller: PatchController,
	plans: readonly CsvImportRowPlan[],
	layerId: string,
	fileName: string,
): Promise<string | null> {
	try {
		const results = await controller.patch.patchFixtures(
			csvImportCandidates(plans, layerId),
		);
		if (!results)
			return "The fixtures could not be imported. Review the Patch status and try again.";
		controller.ui.setStatus(
			`Imported ${results.length} fixture${results.length === 1 ? "" : "s"} from ${fileName}.`,
		);
		controller.ui.setCsvImportOpen(false);
		return null;
	} catch (error) {
		return error instanceof Error
			? error.message
			: "The fixtures could not be imported.";
	}
}
