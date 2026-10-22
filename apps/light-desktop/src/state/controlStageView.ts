import type { StageView } from "../types";

/** Read legacy full-render preferences as the lightweight Control wireframe view. */
export function normalizeControlStageView(
	value: unknown,
	fallback: StageView = "2d",
): "2d" | "3d" {
	if (value === "3d" || value === "3d-viz") return "3d";
	if (value === "2d") return "2d";
	return fallback === "2d" ? "2d" : "3d";
}
