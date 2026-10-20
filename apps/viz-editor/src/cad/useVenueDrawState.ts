import { useEffect, useRef, useState } from "react";
import type { CadTools } from "./cadTools";
import type { CadViewportContext } from "./useCadViewportInteraction";
import type { PlanPoint } from "./projection";
import { planeDelta } from "./types";
import { snapWallEndpoint, VENUE_DRAW_KINDS } from "./venueDrawingGeometry";
import { snapDrawnWall } from "./venueWallSnap";
type Pointer = React.PointerEvent<HTMLCanvasElement>;
type World = [number, number, number];
export function useVenueDrawState(
	context: CadViewportContext,
	tools: CadTools,
) {
	const active = context.editEnabled && VENUE_DRAW_KINDS.includes(tools.tool);
	const [start, setStart] = useState<World | null>(null);
	const startRef = useRef<World | null>(null);
	const first = useRef<World | null>(null);
	const count = useRef(0);
	const [cursor, setCursor] = useState<PlanPoint | null>(null);
	const [cursorShift, setCursorShift] = useState(false);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const queue = useRef(Promise.resolve());
	const pending = useRef(0);
	const consumed = useRef(false);
	const threshold = Math.max(50, 12 / context.camera.zoom);
	function finish() {
		startRef.current = null;
		first.current = null;
		count.current = 0;
		setStart(null);
		setCursor(null);
	}
	useEffect(finish, [tools.tool, context.view, context.rotationQuarterTurns]);
	useEffect(() => {
		if (!active) return;
		const key = (event: KeyboardEvent) => {
			if ((event.target as Element | null)?.closest?.("input,textarea,select"))
				return;
			if (event.key !== "Escape" && event.key !== "Enter") return;
			event.preventDefault();
			finish();
			tools.setTool("select");
		};
		window.addEventListener("keydown", key);
		return () => window.removeEventListener("keydown", key);
	});
	function enqueue(task: () => Promise<void>) {
		pending.current++;
		setBusy(true);
		setError(null);
		queue.current = queue.current
			.then(task)
			.catch((reason) => {
				const message = String(
					reason instanceof Error ? reason.message : reason,
				);
				setError(message);
				tools.reportError?.(message);
				finish();
			})
			.finally(() => {
				pending.current--;
				setBusy(pending.current > 0);
			});
	}
	function point(event: Pointer): PlanPoint {
		const bounds = context.canvas.current?.getBoundingClientRect();
		if (!bounds) return [0, 0];
		return [
			(event.clientX - bounds.left - bounds.width / 2) / context.camera.zoom -
				context.camera.pan[0],
			-(event.clientY - bounds.top - bounds.height / 2) / context.camera.zoom -
				context.camera.pan[1],
		];
	}
	function world(plan: PlanPoint, shift = false): World {
		let value = planeDelta(
			plan,
			context.view,
			context.rotationQuarterTurns,
		).map((v) => Math.round(v) + 0) as World;
		if (shift) return value;
		if (tools.tool === "wall")
			return snapDrawnWall(
				value,
				startRef.current,
				context.entities,
				threshold,
				count.current >= 1 ? first.current : null,
			);
		if (context.snapping)
			value = snapWallEndpoint(value, context.entities, threshold);
		return value;
	}
	return {
		active,
		start,
		setStart,
		startRef,
		first,
		count,
		cursor,
		setCursor,
		cursorShift,
		setCursorShift,
		busy,
		error,
		setError,
		consumed,
		threshold,
		finish,
		enqueue,
		point,
		world,
	};
}
export type VenueDrawState = ReturnType<typeof useVenueDrawState>;
