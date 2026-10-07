import {
	buildSemanticPerformanceWorkload,
	type SemanticPerformanceWorkload,
	type SemanticWorkloadRequest,
	validateSemanticWorkload,
} from "../../../tools/semantic-performance-workload.mjs";
import { readPatchSnapshot } from "../../support/operator/patch";
import type { ApiDriver } from "../core/api";

/**
 * Read-only bridge for the TL-548/TL-553 runner: derive the deterministic semantic workload from
 * the desk's actual patch (root and logical-head identities, position_master mounts). It does
 * not create, start or measure anything; the runner owns installation, paired runs and gates.
 */
export async function semanticWorkloadForLivePatch(
	api: ApiDriver,
	options: { seed: string | number; request?: SemanticWorkloadRequest; showId?: string },
): Promise<SemanticPerformanceWorkload> {
	const patch = await readPatchSnapshot(api, options.showId);
	const workload = buildSemanticPerformanceWorkload(patch, {
		seed: options.seed,
		request: options.request,
	});
	const errors = validateSemanticWorkload(workload);
	if (errors.length > 0)
		throw new Error(`Semantic workload is not contract-valid:\n${errors.join("\n")}`);
	return workload;
}
