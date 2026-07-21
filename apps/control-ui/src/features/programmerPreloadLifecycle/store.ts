export type ProgrammerPreloadLifecycleMutation =
	| "enter"
	| "go"
	| "clear_pending"
	| "release";

export interface ProgrammerPreloadLifecyclePending {
	requestId: string;
	action: ProgrammerPreloadLifecycleMutation;
	optimisticActive: boolean;
}

export interface ProgrammerPreloadLifecycleState {
	showId: string | null;
	userId: string | null;
	deskId: string | null;
	authorityKey: string;
	pending: ProgrammerPreloadLifecyclePending | null;
	error: Error | null;
}

export class ProgrammerPreloadLifecycleStore {
	private readonly listeners = new Set<() => void>();
	private scope = 0;
	private state = emptyState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(
		showId: string | null,
		userId: string | null,
		deskId: string | null,
		authorityKey: string,
	) {
		if (
			showId === this.state.showId &&
			userId === this.state.userId &&
			deskId === this.state.deskId &&
			authorityKey === this.state.authorityKey
		)
			return;
		this.scope++;
		this.state = {
			...emptyState(),
			showId,
			userId,
			deskId,
			authorityKey,
		};
		this.emit();
	}

	begin(
		pending: ProgrammerPreloadLifecyclePending,
		expectedScope = this.scope,
	) {
		if (!this.isScopeCurrent(expectedScope) || this.state.pending) return false;
		this.publish({ pending, error: null });
		return true;
	}

	settle(requestId: string, expectedScope = this.scope) {
		if (!this.matchesPending(requestId, expectedScope)) return false;
		this.publish({ pending: null, error: null });
		return true;
	}

	rollback(
		requestId: string,
		error: Error,
		expectedScope = this.scope,
	) {
		if (!this.matchesPending(requestId, expectedScope)) return false;
		this.publish({ pending: null, error });
		return true;
	}

	abandon(requestId: string, expectedScope = this.scope) {
		if (!this.matchesPending(requestId, expectedScope)) return false;
		this.publish({ pending: null });
		return true;
	}

	captureScope() {
		return this.scope;
	}

	isScopeCurrent(scope: number) {
		return scope === this.scope;
	}

	private matchesPending(requestId: string, expectedScope: number) {
		return (
			this.isScopeCurrent(expectedScope) &&
			this.state.pending?.requestId === requestId
		);
	}

	private publish(update: Partial<ProgrammerPreloadLifecycleState>) {
		this.state = { ...this.state, ...update };
		this.emit();
	}

	private emit() {
		for (const listener of this.listeners) listener();
	}
}

function emptyState(): ProgrammerPreloadLifecycleState {
	return {
		showId: null,
		userId: null,
		deskId: null,
		authorityKey: "",
		pending: null,
		error: null,
	};
}
