import type { PlaybackRuntimeStore } from "../playbackRuntime/store";
import type {
	ProgrammerPreloadLifecycleActions,
	ProgrammerPreloadLifecycleOutcome,
	ProgrammerPreloadLifecycleScope,
} from "./contracts";
import type { ProgrammerPreloadLifecycleWriter } from "./writer";

interface DeskRuntimeAuthority {
	store: PlaybackRuntimeStore;
	activateDesk(): () => void;
}

interface ProgrammerPreloadLifecycleControllerOptions {
	scope: ProgrammerPreloadLifecycleScope;
	writer: ProgrammerPreloadLifecycleWriter;
	runtime: DeskRuntimeAuthority;
	onError?: (error: Error | null) => void;
}

/** Serializes lifecycle gestures and owns the one-shot desk authority for GO. */
export class ProgrammerPreloadLifecycleController
	implements ProgrammerPreloadLifecycleActions
{
	private stopped = false;
	private jobs = 0;
	private tail = Promise.resolve();
	private readonly cancelRuntimeWaits = new Set<(error: Error) => void>();

	constructor(
		private readonly options: ProgrammerPreloadLifecycleControllerOptions,
	) {}

	enter(requestId: string = crypto.randomUUID()) {
		return this.enqueue(() => this.options.writer.enter(requestId));
	}

	go(requestId: string = crypto.randomUUID()) {
		return this.enqueue(() => this.goWithDeskAuthority(requestId));
	}

	clearPending(requestId: string = crypto.randomUUID()) {
		return this.enqueue(() => this.options.writer.clearPending(requestId));
	}

	release(requestId: string = crypto.randomUUID()) {
		return this.enqueue(() => this.options.writer.release(requestId));
	}

	stop() {
		if (this.stopped) return;
		this.stopped = true;
		this.options.writer.stop();
		const error = new Error("Preload lifecycle authority was replaced");
		for (const cancel of this.cancelRuntimeWaits) cancel(error);
		this.cancelRuntimeWaits.clear();
	}

	private enqueue(
		run: () => Promise<ProgrammerPreloadLifecycleOutcome | null>,
	) {
		const invoke = () => (this.stopped ? Promise.resolve(null) : run());
		this.jobs++;
		const result = this.jobs === 1 ? invoke() : this.tail.then(invoke);
		const settled = result.finally(() => {
			this.jobs--;
		});
		this.tail = settled.then(
			() => undefined,
			() => undefined,
		);
		return settled;
	}

	private async goWithDeskAuthority(requestId: string) {
		const release = this.options.runtime.activateDesk();
		const runtimeScope = this.options.runtime.store.captureScope();
		try {
			// The runtime session marks the store loading in its queued refresh.
			await Promise.resolve();
			await this.waitForRuntime(runtimeScope);
			if (this.stopped) return null;
			return await this.options.writer.go(requestId);
		} catch (reason) {
			if (!this.stopped) this.options.onError?.(asError(reason));
			return null;
		} finally {
			release();
		}
	}

	private waitForRuntime(runtimeScope: number) {
		return new Promise<void>((resolve, reject) => {
			let unsubscribe: () => void = () => undefined;
			const settle = (error?: Error) => {
				unsubscribe();
				this.cancelRuntimeWaits.delete(cancel);
				if (error) reject(error);
				else resolve();
			};
			const cancel = (error: Error) => settle(error);
			const inspect = () => {
				const store = this.options.runtime.store;
				const state = store.getSnapshot();
				if (!store.isScopeCurrent(runtimeScope))
					return settle(new Error("Playback runtime authority was replaced"));
				if (
					state.showId !== this.options.scope.showId ||
					state.deskId !== this.options.scope.deskId
				)
					return settle(new Error("Playback runtime scope was replaced"));
				if (state.status === "error")
					return settle(state.error ?? new Error("Playback runtime failed"));
				if (
					state.status === "ready" &&
					state.desk &&
					state.showRevision !== null &&
					state.eventSequence !== null
				)
					settle();
			};
			this.cancelRuntimeWaits.add(cancel);
			unsubscribe = this.options.runtime.store.subscribe(inspect);
			inspect();
		});
	}
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}
