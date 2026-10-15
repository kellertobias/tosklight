// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { DynamicSpatialPreviewResponse } from "../../api/types";
import { createDefaultDynamicDefinition } from "./DynamicsEditor";
import { DynamicPhaseRankPreview } from "./DynamicPreview";

afterEach(cleanup);
function fixture() {
	const body = createDefaultDynamicDefinition(149, "intensity");
	body.target_binding = { type: "live_group", group_id: "25" };
	body.phase_mode = "per_lane";
	body.lanes[0].phase = { ...body.phase, ordering: { type: "source_order" } };
	return { dynamic: { kind: "dynamic" as const, id: body.id, revision: 7, updated_at: "", body }, lane: body.lanes[0] };
}
function response(rank: number): DynamicSpatialPreviewResponse {
	return { show_id: "show", show_revision: 1, dynamic_id: "dynamic", dynamic_revision: 7,
		target_binding: { type: "live_group", group_id: "25" }, base: { type: "live_group", group_id: "25", mapping_provenance: { type: "none" } },
		draft: { projection: { type: "inherit" }, shape: { type: "inherit" } }, inherited_mapping: null,
		source_order: ["fixture-1"], ordered_fixture_ids: ["fixture-1"], projected_positions: [],
		ranks: [{ fixture_id: "fixture-1", rank }], rank_count: rank + 1, warnings: [] };
}
it("requests the selected lane's saved Phase ordering and shows source ranks without inventing a plane", async () => {
	const props = fixture();
	const loadPreview = vi.fn(async () => response(0));
	render(<DynamicPhaseRankPreview {...props} loadPreview={loadPreview} />);
	await waitFor(() => expect(loadPreview).toHaveBeenCalledOnce());
	expect(loadPreview.mock.calls[0]).toEqual([props.dynamic.body.spatial_mapping, props.lane.phase?.ordering]);
	expect(await screen.findByText("Target 1: Rank 1")).toBeInTheDocument();
	expect(screen.getByText("Source order; no spatial plane is used.")).toBeInTheDocument();
	expect(screen.queryByText(/400 virtual/)).not.toBeInTheDocument();
});
it("does not let a stale ordering response replace a newer selected lane preview", async () => {
	const props = fixture();
	let resolveOld: ((value: DynamicSpatialPreviewResponse) => void) | undefined;
	const loadPreview = vi.fn().mockImplementationOnce(() => new Promise<DynamicSpatialPreviewResponse>((resolve) => { resolveOld = resolve; }))
		.mockResolvedValue(response(1));
	const view = render(<DynamicPhaseRankPreview {...props} loadPreview={loadPreview} />);
	await waitFor(() => expect(loadPreview).toHaveBeenCalledOnce());
	const nextLane = { ...props.lane, id: "lane-2", phase: { ...props.dynamic.body.phase, ordering: { type: "grid_linear" as const, angle_degrees: 45 } } };
	view.rerender(<DynamicPhaseRankPreview dynamic={props.dynamic} lane={nextLane} loadPreview={loadPreview} />);
	expect(await screen.findByText("Target 1: Rank 2")).toBeInTheDocument();
	await act(async () => { resolveOld?.(response(0)); });
	expect(screen.queryByText("Target 1: Rank 1")).not.toBeInTheDocument();
});
it("gives targetless guidance without requesting or inventing actual rig ranks", () => {
	const props = fixture();
	props.dynamic.body.target_binding = { type: "targetless" };
	const loadPreview = vi.fn();
	render(<DynamicPhaseRankPreview {...props} loadPreview={loadPreview} />);
	expect(screen.getByText("Choose a saved Group or frozen targets to preview Phase ranks.")).toBeInTheDocument();
	expect(loadPreview).not.toHaveBeenCalled();
});

it.each([
	[{ type: "random_each_loop", seed: 9 }, "Random ranks; no spatial plane is used."],
	[{ type: "grid_linear", angle_degrees: 45 }, "Spatial positions unavailable; showing authoritative ranks."],
] as const)("does not label non-spatial or unavailable-position ranks as source order: %j", async (ordering, guidance) => {
	const props = fixture();
	props.lane.phase = { ...props.dynamic.body.phase, ordering };
	render(<DynamicPhaseRankPreview {...props} loadPreview={async () => response(0)} />);
	expect(await screen.findByText(guidance)).toBeInTheDocument();
	expect(screen.queryByText("Source order; no spatial plane is used.")).not.toBeInTheDocument();
});
