import type { CadTools } from "./cadTools";
import type { PlanPoint } from "./projection";
import type { CadViewportContext } from "./useCadViewportInteraction";
import type { VenueDrawState } from "./useVenueDrawState";
import { openingPreview, pickWallOpening } from "./venueDrawingGeometry";
import {
	placeDrawnObject,
	placeWallOpening,
	placeWallSegment,
} from "./venueDrawPersistence";
type Pointer = React.PointerEvent<HTMLCanvasElement>;
function selectOpening(
	context: CadViewportContext,
	tools: CadTools,
	event: Pointer,
	plan: PlanPoint,
) {
	const hit = pickWallOpening(
		context.entities,
		plan,
		context.view,
		context.rotationQuarterTurns,
		Math.max(30, 5 / context.camera.zoom),
	);
	tools.selectOpening?.(
		hit
			? { fixtureId: hit.wall.logicalFixtureId, openingId: hit.opening.id }
			: null,
	);
	if (!hit) return false;
	tools.selectText(null);
	context.onFocusEntity?.(hit.wall.id);
	context.onSelection({
		type: event.shiftKey ? "add" : "replace",
		ids: [hit.wall.logicalFixtureId],
	});
	return true;
}
export function venuePointerDown(
	context: CadViewportContext,
	tools: CadTools,
	state: VenueDrawState,
	event: Pointer,
) {
	const {
		active,
		consumed,
		setCursor,
		setError,
		threshold,
		enqueue,
		point,
		world,
		first,
		startRef,
		setStart,
		count,
		finish,
	} = state;

	if (
		!context.editEnabled ||
		event.button !== 0 ||
		event.altKey ||
		event.ctrlKey
	)
		return false;
	const plan = point(event);
	if (tools.tool === "select")
		return selectOpening(context, tools, event, plan);
	if (!active) return false;
	consumed.current = true;
	setCursor(plan);
	setError(null);
	if (tools.tool === "window" || tools.tool === "door") {
		const preview = openingPreview(
			context.entities,
			plan,
			tools.tool,
			context.view,
			context.rotationQuarterTurns,
			threshold,
		);
		if (!preview?.valid) {
			setError(
				preview?.reason ??
					"Move the pointer onto a wall to place this opening.",
			);
			return true;
		}
		enqueue(async () => {
			const id = await placeWallOpening(preview);
			tools.selectOpening?.({
				fixtureId: preview.wall.logicalFixtureId,
				openingId: id,
			});
			tools.announcePlaced(preview.wall.logicalFixtureId);
		});
		return true;
	}
	if (context.view !== "top_down") {
		setError("Use a top view to draw walls or place pillars and stairs.");
		return true;
	}
	const next = world(plan, event.shiftKey);
	if (tools.tool === "wall") {
		const previous = startRef.current;
		if (!previous) {
			first.current = next;
			startRef.current = next;
			setStart(next);
			return true;
		}
		if (Math.hypot(next[0] - previous[0], next[1] - previous[1]) < 10)
			return true;
		enqueue(async () => {
			tools.announcePlaced(await placeWallSegment(previous, next));
		});
		count.current++;
		if (
			first.current &&
			next[0] === first.current[0] &&
			next[1] === first.current[1]
		) {
			finish();
			tools.setTool("select");
		} else {
			startRef.current = next;
			setStart(next);
		}
		return true;
	}
	const kind = tools.tool;
	if (kind === "pillar" || kind === "stairs")
		enqueue(async () => {
			tools.announcePlaced(await placeDrawnObject(kind, next));
		});
	return true;
}
