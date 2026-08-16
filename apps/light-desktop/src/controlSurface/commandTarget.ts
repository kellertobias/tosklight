export type CommandTargetMode = "FIXTURE" | "GROUP";

export function commandTargetAfterEnter(
	command: string,
	target: CommandTargetMode,
	pristine: boolean,
): CommandTargetMode | null {
	if (pristine || command.trim().toUpperCase() !== "DEGROUP") return null;
	return target === "GROUP" ? "FIXTURE" : "GROUP";
}

export function defaultCommandLine(target: CommandTargetMode): string {
	return target;
}
