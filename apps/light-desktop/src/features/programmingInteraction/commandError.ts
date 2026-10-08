/** Scoped command feedback. A null detail retires the current error on the next edit. */
export const COMMAND_ERROR_EVENT = "light:command-error";

export function reportProgrammingCommandError(error: Error | null): void {
	if (!error) return;
	window.dispatchEvent(
		new CustomEvent<string | null>(COMMAND_ERROR_EVENT, {
			detail: error.message,
		}),
	);
}

export function clearProgrammingCommandError(): void {
	window.dispatchEvent(
		new CustomEvent<null>(COMMAND_ERROR_EVENT, { detail: null }),
	);
}
