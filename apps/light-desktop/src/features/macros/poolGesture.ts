export type MacroPoolGestureOutcome = "create" | "edit" | "run";

/** Resolve one Macro pool gesture before the window performs any transport action. */
export function resolveMacroPoolGesture(
	occupied: boolean,
	setActive: boolean,
	secondary: boolean,
): MacroPoolGestureOutcome {
	if (!occupied) return "create";
	if (setActive || secondary) return "edit";
	return "run";
}
