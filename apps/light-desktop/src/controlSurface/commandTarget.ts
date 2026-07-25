export type CommandTargetMode = "FIXTURE" | "GROUP";

export function commandTargetAfterEnter(
	command: string,
	target: CommandTargetMode,
	pristine: boolean,
): CommandTargetMode | null {
	const opposite = target === "GROUP" ? "FIXTURE" : "GROUP";
	if (pristine || command.trim().toUpperCase() !== opposite) return null;
	return target === "GROUP" ? "FIXTURE" : "GROUP";
}

export function defaultCommandLine(target: CommandTargetMode): string {
	return target;
}
