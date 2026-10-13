/** Actual responses are shared by surfaces, scoped to the addressed connection and output. */
export interface MediaPreviewScope {
	server: string;
	output: string | null;
	connection: string;
}
export interface MediaPreviewOutcome {
	scope: MediaPreviewScope;
	state: "missing" | "offline" | "recovered";
	error?: string;
	serverWide?: boolean;
}
function serverKey(scope: MediaPreviewScope) {
	return JSON.stringify([scope.server, scope.connection]);
}
function outputKey(scope: MediaPreviewScope) {
	return JSON.stringify([scope.server, scope.connection, scope.output]);
}
export function affectedPreview(
	scope: MediaPreviewScope,
	outcome: MediaPreviewOutcome,
) {
	return (
		serverKey(scope) === serverKey(outcome.scope) &&
		(outcome.state === "offline" ||
			scope.output === outcome.scope.output ||
			outcome.serverWide === true)
	);
}

/** The show-owned actions provider holds this authority; it never polls or writes output. */
export class MediaPreviewAuthority {
	private listeners = new Set<(outcome: MediaPreviewOutcome) => void>();
	private epochs = new Map<string, number>();
	private failures = new Map<string, MediaPreviewOutcome>();
	subscribe(listener: (outcome: MediaPreviewOutcome) => void) {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}
	token(scope: MediaPreviewScope) {
		return `${this.epochs.get(serverKey(scope)) ?? 0}:${this.epochs.get(outputKey(scope)) ?? 0}`;
	}
	current(scope: MediaPreviewScope, token: string) {
		return this.token(scope) === token;
	}
	failure(
		scope: MediaPreviewScope,
		state: "missing" | "offline",
		error: string,
		token: string,
	) {
		if (!this.current(scope, token)) return;
		const key = state === "offline" ? serverKey(scope) : outputKey(scope);
		this.epochs.set(key, (this.epochs.get(key) ?? 0) + 1);
		const outcome = { scope, state, error };
		this.failures.set(key, outcome);
		this.emit(outcome);
	}
	success(scope: MediaPreviewScope, token: string) {
		if (!this.current(scope, token)) return;
		for (const key of [serverKey(scope), outputKey(scope)]) {
			const failed = this.failures.get(key);
			if (!failed) continue;
			this.failures.delete(key);
			this.epochs.set(key, (this.epochs.get(key) ?? 0) + 1);
			this.emit({
				scope: failed.state === "offline" ? { ...scope, output: null } : scope,
				state: "recovered",
				serverWide: failed.state === "offline",
			});
		}
	}
	private emit(outcome: MediaPreviewOutcome) {
		for (const listener of this.listeners) listener(outcome);
	}
}
