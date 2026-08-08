import { useEffect, useState } from "react";
import type { DynamicSpatialMappingOverrideProjection } from "../../api/types";
import type { ShowObject } from "../../features/showObjects/contracts";
import type { ProjectedPlanePositions } from "./DynamicPreview";
import type { DynamicEditorProps } from "./DynamicsEditor";

/**
 * The plane the saved mapping ranks on, asked of the desk rather than recomputed here.
 *
 * The Projection tab drives its own request from the draft the operator is turning. This one
 * follows what is stored, because that is what the Phase spread and every running instance
 * actually order by.
 */
export function useProjectedPlane(
	dynamic: ShowObject<"dynamic">,
	loadPreview: DynamicEditorProps["onLoadSpatialPreview"],
): ProjectedPlanePositions | undefined {
	const [plane, setPlane] = useState<ProjectedPlanePositions | undefined>(
		undefined,
	);
	const mapping = dynamic.body
		.spatial_mapping as DynamicSpatialMappingOverrideProjection;
	const signature = JSON.stringify(mapping ?? null);
	useEffect(() => {
		if (!loadPreview || !mapping) return;
		let cancelled = false;
		void loadPreview(mapping)
			.then((preview) => {
				if (cancelled) return;
				const placed = preview.projected_positions.filter(
					(position) => position.u != null && position.v != null,
				);
				// No projection means source order, which has no plane to plot on. Dropping the
				// map lets the preview fall back to Stage positions instead of collapsing.
				setPlane(
					placed.length
						? new Map(
								placed.map((position) => [
									position.fixture_id,
									{ u: position.u as number, v: position.v as number },
								]),
							)
						: undefined,
				);
			})
			.catch(() => {
				if (!cancelled) setPlane(undefined);
			});
		return () => {
			cancelled = true;
		};
		// The mapping is compared by content: a re-fetch is worth it only when it really changed.
	}, [dynamic.id, signature, loadPreview, mapping]);
	return plane;
}
