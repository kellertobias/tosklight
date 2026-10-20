import { useCadTools } from "./cadTools";
import type { CadViewportContext } from "./useCadViewportInteraction";
import type { PlanPoint } from "./projection";
import { useVenueDrawState } from "./useVenueDrawState";
import { venuePointerDown } from "./venueDrawingActions";
import { venueToolPreview } from "./venueToolPreview";
type Pointer = React.PointerEvent<HTMLCanvasElement>;
export interface VenueToolPreview {
	kind: string;
	points: PlanPoint[];
	valid: boolean;
	wallId?: string;
}
export function useCadVenueTool(context: CadViewportContext) {
	const tools = useCadTools();
	const state = useVenueDrawState(context, tools);
	const {
		active,
		start,
		cursor,
		world,
		threshold,
		busy,
		error,
		setCursor,
		point,
		consumed,
		finish,
	} = state;
	const preview = active
		? venueToolPreview(
				context,
				tools.tool,
				start,
				cursor,
				(point) => world(point, state.cursorShift),
				threshold,
			)
		: null;
	const pointerDown = (event: Pointer) =>
		venuePointerDown(context, tools, state, event);
	const instruction =
		tools.tool === "wall"
			? start
				? "Click the next wall endpoint. Click the first point to close, or press Escape to finish."
				: "Click the first wall endpoint, then click connected corners."
			: tools.tool === "window" || tools.tool === "door"
				? `Move onto a wall and click to place a ${tools.tool}.`
				: `Click to place ${tools.tool}.`;
	return {
		active,
		preview,
		busy,
		message:
			error ??
			(context.view !== "top_down" &&
			tools.tool !== "window" &&
			tools.tool !== "door"
				? "Use a top view for this venue tool."
				: instruction),
		error,
		pointerDown,
		pointerMove: (event: Pointer) => {
			if (active) {
				setCursor(point(event));
				state.setCursorShift(event.shiftKey);
			}
		},
		pointerUp: () => {
			const handled = consumed.current;
			consumed.current = false;
			return handled;
		},
		finish,
		doubleClick: () => {
			if (active) {
				finish();
				tools.setTool("select");
			}
		},
		contextMenu: (event: React.MouseEvent<HTMLCanvasElement>) => {
			if (!active) return false;
			event.preventDefault();
			finish();
			tools.setTool("select");
			return true;
		},
		stop: () => {
			finish();
			tools.setTool("select");
		},
	};
}
