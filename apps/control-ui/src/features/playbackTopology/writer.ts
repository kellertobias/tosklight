import type { CueList, PlaybackDefinition } from "../../api/types";
import type { ShowObject, ShowObjectKind } from "../showObjects/contracts";
import type { ShowObjectsStore } from "../showObjects/store";
import { repairPlaybackTopologyConflict } from "./conflictRepair";
import type {
	ExistingPlaybackPageRevisionBasis,
	ExistingPlaybackRevisionBasis,
	PlaybackPageRevisionBasis,
	PlaybackTopologyAction,
	PlaybackTopologyActions,
	PlaybackTopologyOutcome,
	PlaybackTopologyRequest,
	PlaybackTopologyRevisionBasis,
	PlaybackTopologyTransport,
} from "./contracts";
import { existingPageRevisions, readyPageRevisions } from "./pageAuthority";
import { normalizePlaybackPageName } from "./pageNames";
import { PlaybackTopologyWriterLifecycle } from "./writerLifecycle";

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
	private readonly lifecycle: PlaybackTopologyWriterLifecycle;

	constructor(private readonly options: PlaybackTopologyWriterOptions) {
		this.lifecycle = new PlaybackTopologyWriterLifecycle(
			options.showId,
			options.store,
			options.transport,
		);
	}

	createPage(page: number, revisionBasis?: PlaybackPageRevisionBasis) {
		return this.lifecycle.enqueue((generation) =>
			this.createPageNow(generation, page, revisionBasis),
		);
	}

	renamePage(
		page: number,
		name: string,
		revisionBasis?: ExistingPlaybackPageRevisionBasis,
	) {
		return this.lifecycle.enqueue((generation) =>
			this.renamePageNow(generation, page, name, revisionBasis),
		);
	}

	saveCueList(
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string | null,
		body: CueList,
	) {
		return this.lifecycle.enqueue((generation) =>
			this.saveCueListNow(
				generation,
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
		return this.lifecycle.enqueue((generation) =>
			this.configureSlotNow(generation, page, slot, playback, revisionBasis),
		);
	}

	mapExistingPlayback(
		page: number,
		slot: number,
		playbackNumber: number,
		revisionBasis?: ExistingPlaybackRevisionBasis,
	) {
		return this.lifecycle.enqueue((generation) =>
			this.mapExistingPlaybackNow(
				generation,
				page,
				slot,
				playbackNumber,
				revisionBasis,
			),
		);
	}

	clearMappedPlayback(
		page: number,
		slot: number,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		return this.lifecycle.enqueue((generation) =>
			this.clearMappedPlaybackNow(generation, page, slot, revisionBasis),
		);
	}

	stop() {
		this.lifecycle.stop();
	}

	private createPageNow(
		generation: number,
		page: number,
		revisionBasis?: PlaybackPageRevisionBasis,
	) {
		const revisions = readyPageRevisions(
			this.options.store,
			page,
			revisionBasis,
		);
		if (!revisions)
			return this.fail("Authoritative Playback Pages are loading", generation);
		return this.apply({ type: "create_page", page, ...revisions }, generation);
	}

	private renamePageNow(
		generation: number,
		page: number,
		name: string,
		revisionBasis?: ExistingPlaybackPageRevisionBasis,
	) {
		const normalized = normalizePlaybackPageName(name);
		if (!normalized)
			return this.fail("Playback Page name is invalid", generation);
		const revisions = existingPageRevisions(
			this.options.store,
			page,
			revisionBasis,
		);
		if (!revisions)
			return this.fail(
				`Authoritative Playback Page ${page} is not available`,
				generation,
			);
		return this.apply(
			{ type: "rename_page", page, name: normalized, ...revisions },
			generation,
		);
	}

	private saveCueListNow(
		generation: number,
		cueListId: string,
		expectedRevision: number,
		expectedObjectId: string | null,
		body: CueList,
	) {
		const snapshot = this.options.store.getSnapshot();
		if (!snapshot.readyCollections.has("cue_list"))
			return this.fail("Authoritative Cuelists are loading", generation);
		return this.apply(
			{
				type: "save_cue_list",
				cueListId,
				expectedRevision,
				expectedObjectId,
				body,
			},
			generation,
		);
	}

	private configureSlotNow(
		generation: number,
		page: number,
		slot: number,
		playback: PlaybackDefinition,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		const revisions = this.readySlotRevisions(page, slot, revisionBasis);
		if (!revisions)
			return this.fail("Authoritative Playback topology is loading", generation);
		return this.apply(
			{ type: "configure_slot", page, slot, ...revisions, playback },
			generation,
		);
	}

	private clearMappedPlaybackNow(
		generation: number,
		page: number,
		slot: number,
		revisionBasis?: PlaybackTopologyRevisionBasis,
	) {
		const revisions = this.readySlotRevisions(page, slot, revisionBasis);
		if (!revisions)
			return this.fail("Authoritative Playback topology is loading", generation);
		return this.apply(
			{ type: "clear_mapped_playback", page, slot, ...revisions },
			generation,
		);
	}

	private mapExistingPlaybackNow(
		generation: number,
		page: number,
		slot: number,
		playbackNumber: number,
		revisionBasis?: ExistingPlaybackRevisionBasis,
	) {
		const revisions = this.existingPlaybackRevisions(
			page,
			playbackNumber,
			revisionBasis,
		);
		if (!revisions)
			return this.fail(
				`Authoritative Playback ${playbackNumber} is not available`,
				generation,
			);
		return this.apply(
			{
				type: "map_existing_playback",
				page,
				slot,
				playbackNumber,
				...revisions,
			},
			generation,
		);
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

	private existingPlaybackRevisions(
		page: number,
		playbackNumber: number,
		revisionBasis?: ExistingPlaybackRevisionBasis,
	): ExistingPlaybackRevisionBasis | null {
		const snapshot = this.options.store.getSnapshot();
		if (
			!snapshot.readyCollections.has("playback") ||
			!snapshot.readyCollections.has("playback_page")
		)
			return null;
		if (revisionBasis) return revisionBasis;
		const pageObject = snapshot.playbackPages.find(
			(object) => object.body.number === page,
		);
		const source = snapshot.playbacks.find(
			(object) => object.body.number === playbackNumber,
		);
		if (!source || source.body.target.type !== "cue_list") return null;
		return {
			expectedPageRevision: pageObject?.revision ?? 0,
			expectedPageObjectId: pageObject?.id ?? null,
			expectedPlaybackRevision: source.revision,
			expectedPlaybackObjectId: source.id,
		};
	}

	private async apply(action: PlaybackTopologyAction, generation: number) {
		if (!this.lifecycle.isCurrent(generation)) return null;
		const snapshot = this.options.store.getSnapshot();
		if (snapshot.showRevision == null)
			return this.fail("Authoritative Show revision is loading", generation);
		const request = { requestId: crypto.randomUUID(), action };
		try {
			const outcome = await this.lifecycle.send(
				snapshot.showRevision,
				request,
				generation,
			);
			if (!this.lifecycle.isCurrent(generation)) return null;
			assertOutcome(request, outcome);
			this.install(outcome);
			this.options.onError?.(null);
			return outcome;
		} catch (reason) {
			if (!this.lifecycle.isCurrent(generation)) return null;
			const error = asError(reason);
			await repairPlaybackTopologyConflict(
				this.options,
				error,
				action,
				generation,
			);
			if (!this.lifecycle.isCurrent(generation)) return null;
			this.options.onError?.(error);
			return null;
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

	private fail(message: string, generation: number): Promise<null> {
		if (this.lifecycle.isCurrent(generation))
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
	if (action.type === "create_page" || action.type === "rename_page") {
		if (resolution.kind !== "page" || resolution.page !== action.page)
			throw new Error("Playback topology response Page does not match");
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
