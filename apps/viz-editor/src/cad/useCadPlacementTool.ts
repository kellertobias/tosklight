/**
 * Repeated placement on a viewport: while **Add Several** holds a Venue element, every press places
 * one more copy where it lands, and Escape puts the element down.
 *
 * A press lands on the plane the view shows, so a plan places on the floor and an elevation at the
 * height pressed; the axis the view looks along stays at zero.
 */
import { useEffect } from "react";
import { useCadTools } from "./cadTools";
import type { CadViewDirection, TileCamera } from "./types";
import { planeDelta } from "./types";

export function useCadPlacementTool({
	canvas,
	view,
	rotationQuarterTurns,
	camera,
	enabled,
}: {
	canvas: React.RefObject<HTMLCanvasElement | null>;
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	enabled: boolean;
}) {
	const tools = useCadTools();
	const active = enabled && tools.placing !== null;
	useEffect(() => {
		if (!tools.placing) return;
		const finish = (event: KeyboardEvent) => {
			if (event.key !== "Escape") return;
			event.preventDefault();
			tools.stopPlacing();
		};
		window.addEventListener("keydown", finish);
		return () => window.removeEventListener("keydown", finish);
	}, [tools]);

	/** Places a copy where a primary press lands; false leaves the press to the other gestures. */
	function pointerDown(event: React.PointerEvent<HTMLCanvasElement>): boolean {
		if (!active || event.button !== 0) return false;
		const bounds = canvas.current?.getBoundingClientRect();
		if (!bounds) return true;
		const plan: [number, number] = [
			(event.clientX - bounds.left - bounds.width / 2) / camera.zoom - camera.pan[0],
			-(event.clientY - bounds.top - bounds.height / 2) / camera.zoom - camera.pan[1],
		];
		const [x, y, z] = planeDelta(plan, view, rotationQuarterTurns);
		// Whole millimetres, with a rounded -0 turned back into 0.
		const metres = (millimetres: number) => Math.round(millimetres) / 1000 + 0;
		void tools.placeAt({ x: metres(x), y: metres(y), z: metres(z) });
		return true;
	}

	return { active, placing: tools.placing, pointerDown, stop: tools.stopPlacing };
}
