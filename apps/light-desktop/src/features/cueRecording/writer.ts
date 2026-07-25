import type { PlaybackRuntimeStore } from "../playbackRuntime/store";
import type {
	ShowObject,
	ShowObjectKind,
} from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	CueRecordingActions,
	CueRecordingOutcome,
	CueRecordingRequest,
	CueRecordingTransport,
	CueRecordTarget,
	RecordCueInput,
} from "./contracts";

export interface CueRecordingWriterOptions {
	showId: string;
	store: ShowObjectsStore;
	playbackRuntimeStore: PlaybackRuntimeStore;
	transport: CueRecordingTransport;
	selectedPlayback(): number | null;
	loadObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObject<K> | null>;
	onError?: (error: Error | null) => void;
}

/** One action request owns capture, topology mutation, event, and optional activation. */
export class CueRecordingWriter implements CueRecordingActions {
	private stopped = false;

	constructor(private readonly options: CueRecordingWriterOptions) {}

	async record(input: RecordCueInput) {
		if (this.stopped) return null;
		const snapshot = this.options.store.getSnapshot();
		const generation = snapshot.authorityGeneration;
		const revision = snapshot.showRevision;
		if (revision == null) return this.fail("Authoritative Show revision is loading");
		const selectedPlayback =
			input.target.kind === "selected_playback"
				? this.options.selectedPlayback()
				: null;
		const request = { ...input, requestId: crypto.randomUUID() };
		try {
			const outcome = await this.send(revision, request);
			if (!this.isCurrent(generation)) return null;
			assertOutcome(request, outcome);
			this.installOutcome(outcome);
			this.options.onError?.(null);
			return outcome;
		} catch (reason) {
			if (!this.isCurrent(generation)) return null;
			const error = asError(reason);
			await this.repairConflict(
				error,
				input.target,
				generation,
				selectedPlayback,
			);
			this.options.onError?.(error);
			return null;
		}
	}

	stop() {
		this.stopped = true;
	}

	private async send(revision: number, request: CueRecordingRequest) {
		try {
			return await this.options.transport.record(
				this.options.showId,
				revision,
				request,
			);
		} catch (reason) {
			if (!transportFailure(reason)?.retryable) throw reason;
			return this.options.transport.record(
				this.options.showId,
				revision,
				request,
			);
		}
	}

	private installOutcome(outcome: CueRecordingOutcome) {
		if (outcome.status === "no_change") {
			this.options.store.installShowRevision(
				this.options.showId,
				outcome.showRevision,
			);
			return;
		}
		const { cueList, playback, page } = outcome.projections;
		const installs = [
			{ kind: "cue_list" as const, objectId: cueList.id, object: cueList },
			...(playback
				? [{ kind: "playback" as const, objectId: playback.id, object: playback }]
				: []),
			...(page
				? [
						{
							kind: "playback_page" as const,
							objectId: page.id,
							object: page,
						},
					]
				: []),
		];
		this.options.store.installObjects(
			this.options.showId,
			installs,
			outcome.showEventSequence,
			outcome.showRevision,
			"seal",
		);
		if (outcome.runtime)
			this.options.playbackRuntimeStore.applyProjection(
				outcome.runtime.projection,
				outcome.runtime.eventSequence,
			);
	}

	private async repairConflict(
		error: Error,
		target: CueRecordTarget,
		generation: number,
		selectedPlayback: number | null,
	) {
		const failure = transportFailure(error);
		if (failure?.status !== 409) return;
		if (failure.currentRevision != null)
			this.options.store.installShowRevision(
				this.options.showId,
				failure.currentRevision,
				generation,
			);
		if (!this.isCurrent(generation)) return;
		await this.repairTarget(target, generation, selectedPlayback);
	}

	private async repairTarget(
		target: CueRecordTarget,
		generation: number,
		selectedPlayback: number | null,
	) {
		if (target.kind === "cue_list")
			return this.repairObject("cue_list", target.cueListId, generation);
		if (target.kind === "pool")
			return this.repairPlayback(target.playbackNumber, generation);
		if (target.kind === "selected_playback") {
			return selectedPlayback == null
				? undefined
				: this.repairPlayback(selectedPlayback, generation);
		}
		const page = await this.repairObject(
			"playback_page",
			String(target.page),
			generation,
		);
		const playback = page?.body.slots[String(target.slot)];
		if (playback != null) await this.repairPlayback(playback, generation);
	}

	private async repairPlayback(number: number, generation: number) {
		const playback = await this.repairObject(
			"playback",
			String(number),
			generation,
		);
		if (playback?.body.target.type === "cue_list")
			await this.repairObject(
				"cue_list",
				playback.body.target.cue_list_id,
				generation,
			);
	}

	private async repairObject<K extends ShowObjectKind>(
		kind: K,
		objectId: string,
		generation: number,
	) {
		const stamp = this.options.store.captureObjectAuthority(
			this.options.showId,
			kind,
			objectId,
		);
		if (!stamp || stamp.authorityGeneration !== generation) return null;
		try {
			const object = await this.options.loadObject(
				this.options.showId,
				kind,
				objectId,
			);
			return this.options.store.installObjectIfAuthorityUnchanged(stamp, object)
				? object
				: null;
		} catch {
			return null;
		}
	}

	private isCurrent(generation: number) {
		return (
			!this.stopped &&
			this.options.store.getSnapshot().authorityGeneration === generation
		);
	}

	private fail(message: string): null {
		this.options.onError?.(new Error(message));
		return null;
	}
}

function assertOutcome(
	request: CueRecordingRequest,
	outcome: CueRecordingOutcome,
) {
	if (request.requestId !== outcome.requestId)
		throw new Error("Cue recording response request ID does not match");
	if (
		request.cueNumber != null &&
		request.cueNumber !== outcome.recordedCue.number
	)
		throw new Error("Cue recording response Cue number does not match");
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}

function transportFailure(reason: unknown) {
	if (!(reason instanceof Error)) return null;
	const failure = reason as Error & {
		status?: unknown;
		retryable?: unknown;
		currentRevision?: unknown;
	};
	if (
		typeof failure.status !== "number" ||
		typeof failure.retryable !== "boolean"
	)
		return null;
	return {
		status: failure.status,
		retryable: failure.retryable,
		currentRevision:
			typeof failure.currentRevision === "number"
				? failure.currentRevision
				: null,
	};
}
