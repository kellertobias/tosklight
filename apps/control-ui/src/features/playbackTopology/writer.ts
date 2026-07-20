import type { CueList, PlaybackDefinition } from "../../api/types";
import type {
	ShowObject,
	ShowObjectKind,
} from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import type {
	PlaybackTopologyAction,
	PlaybackTopologyActions,
	PlaybackTopologyOutcome,
	PlaybackTopologyRequest,
	PlaybackTopologyRevisionBasis,
	PlaybackTopologyTransport,
} from "./contracts";

export interface PlaybackTopologyWriterOptions {
	showId: string;
	store: ShowObjectsStore;
	transport: PlaybackTopologyTransport;
	loadObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	): Promise<ShowObject<K> | null>;
	onError?: (error: Error | null) => void;
}

/** Serializes each portable topology intent as one revision-checked Show action. */
export class PlaybackTopologyWriter implements PlaybackTopologyActions {
	private stopped = false;
	private tail: Promise<void> = Promise.resolve();

	constructor(private readonly options: PlaybackTopologyWriterOptions) {}

	saveCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string | null,
		body: CueList,
	) {
		return this.enqueue(() =>
			this.saveCueListNow(
				cueListId,
				expectedRevision,
				expectedObjectId,
				body,
			),
		);
	}

	configureSlot(
		page: number,
		slot: number,
		playback: PlaybackDefinition,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		return this.enqueue(() =>
			this.configureSlotNow(page, slot, playback, revisionBasis),
		);
	}

	clearMappedPlayback(
		page: number,
		slot: number,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		return this.enqueue(() =>
			this.clearMappedPlaybackNow(page, slot, revisionBasis),
		);
	}

	stop() {
		this.stopped = true;
	}

	private saveCueListNow(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string | null,
		body: CueList,
	) {
		const snapshot = this.options.store.getSnapshot();
		if (!snapshot.readyCollections.has("cue_list"))
			return this.fail("Authoritative Cuelists are loading");
		return this.apply({
			type: "save_cue_list",
			cueListId,
			expectedRevision,
			expectedObjectId,
			body,
		});
	}

	private configureSlotNow(
		page: number,
		slot: number,
		playback: PlaybackDefinition,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		const revisions = this.readySlotRevisions(page, slot, revisionBasis);
		if (!revisions) return this.fail("Authoritative Playback topology is loading");
		return this.apply({
			type: "configure_slot",
			page,
			slot,
			...revisions,
			playback,
		});
	}

	private clearMappedPlaybackNow(
		page: number,
		slot: number,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		const revisions = this.readySlotRevisions(page, slot, revisionBasis);
		if (!revisions) return this.fail("Authoritative Playback topology is loading");
		return this.apply({
			type: "clear_mapped_playback",
			page,
			slot,
			...revisions,
		});
	}

	private enqueue(operation: () => Promise<PlaybackTopologyOutcome | null>) {
		const result = this.tail.then(operation, operation);
		this.tail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	private slotRevisions(page: number, slot: number) {
		const snapshot = this.options.store.getSnapshot();
		if (
			!snapshot.readyCollections.has("playback") ||
			!snapshot.readyCollections.has("playback_page")
		)
			return null;
		const pageObject = snapshot.playbackPages.find(
			(object) => object.body.number === page,
		);
		const playbackNumber = pageObject?.body.slots[String(slot)];
		const playback = snapshot.playbacks.find(
			(object) => object.body.number === playbackNumber,
		);
		return {
			expectedPageRevision: pageObject?.revision ?? 0,
			expectedPageObjectId: pageObject?.id ?? null,
			expectedPlaybackRevision: playback?.revision ?? 0,
			expectedPlaybackObjectId: playback?.id ?? null,
		};
	}

	private readySlotRevisions(
		page: number,
		slot: number,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		const current = this.slotRevisions(page, slot);
		return current ? (revisionBasis ?? current) : null;
	}

	private async apply(action: PlaybackTopologyAction) {
		if (this.stopped) return null;
		const snapshot = this.options.store.getSnapshot();
		const generation = snapshot.authorityGeneration;
		if (snapshot.showRevision == null)
			return this.fail("Authoritative Show revision is loading");
		const request = { requestId: crypto.randomUUID(), action };
		try {
			const outcome = await this.send(snapshot.showRevision, request);
			if (!this.isCurrent(generation)) return null;
			assertOutcome(request, outcome);
			this.install(outcome);
			this.options.onError?.(null);
			return outcome;
		} catch (reason) {
			if (!this.isCurrent(generation)) return null;
			const error = asError(reason);
			await this.repairConflict(error, action, generation);
			this.options.onError?.(error);
			return null;
		}
	}

	private async send(
		revision: number,
		request: PlaybackTopologyRequest,
	): Promise<PlaybackTopologyOutcome> {
		try {
			return await this.options.transport.apply(
				this.options.showId,
				revision,
				request,
			);
		} catch (reason) {
			if (!transportFailure(reason)?.retryable) throw reason;
			return this.options.transport.apply(this.options.showId, revision, request);
		}
	}

	private install(outcome: PlaybackTopologyOutcome) {
		const installs = outcome.objects.map((object) => ({
			kind: object.kind,
			objectId: object.objectId,
			object:
				object.state === "deleted"
					? null
					: ({
							kind: object.kind,
							id: object.objectId,
							revision: object.objectRevision,
							updated_at: "",
							body: object.body,
						} as ShowObject),
		}));
		this.options.store.installObjects(
			this.options.showId,
			installs,
			outcome.status === "changed" ? outcome.eventSequence : null,
			outcome.showRevision,
			outcome.status === "changed" ? "seal" : "floor",
		);
	}

	private async repairConflict(
		error: Error,
		action: PlaybackTopologyAction,
		generation: number,
	) {
		const failure = transportFailure(error);
		if (failure?.status !== 409) return;
		if (failure.currentRevision != null)
			this.options.store.installShowRevision(
				this.options.showId,
				failure.currentRevision,
				generation,
			);
		if (action.type === "save_cue_list") {
			const stale = this.options.store
				.getSnapshot()
				.cueLists.find((object) => object.body.id === action.cueListId);
			return this.repairObject(
				"cue_list",
				stale?.id ?? action.cueListId,
				generation,
			);
		}
		const before = this.options.store.getSnapshot();
		const stalePage = before.playbackPages.find(
			(object) => object.body.number === action.page,
		);
		const staleNumber = stalePage?.body.slots[String(action.slot)];
		const stalePlayback = before.playbacks.find(
			(object) => object.body.number === staleNumber,
		);
		const page = await this.repairObject(
			"playback_page",
			stalePage?.id ?? String(action.page),
			generation,
		);
		if (stalePlayback)
			await this.repairObject("playback", stalePlayback.id, generation);
		const playbackNumber = page?.body.slots[String(action.slot)];
		if (playbackNumber == null) return;
		const currentPlayback = this.options.store
			.getSnapshot()
			.playbacks.find((object) => object.body.number === playbackNumber);
		if (currentPlayback?.id !== stalePlayback?.id)
			await this.repairObject(
				"playback",
				currentPlayback?.id ?? String(playbackNumber),
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

	private fail(message: string): Promise<null> {
		this.options.onError?.(new Error(message));
		return Promise.resolve(null);
	}
}

function assertOutcome(
	request: PlaybackTopologyRequest,
	outcome: PlaybackTopologyOutcome,
) {
	if (request.requestId !== outcome.requestId)
		throw new Error("Playback topology response request ID does not match");
	const action = request.action;
	const resolution = outcome.resolution;
	if (action.type === "save_cue_list") {
		if (
			resolution.kind !== "cue_list" ||
			resolution.cueListId !== action.cueListId
		)
			throw new Error("Playback topology response Cuelist does not match");
		return;
	}
	if (
		resolution.kind !== "page_slot" ||
		resolution.page !== action.page ||
		resolution.slot !== action.slot
	)
		throw new Error("Playback topology response slot does not match");
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
