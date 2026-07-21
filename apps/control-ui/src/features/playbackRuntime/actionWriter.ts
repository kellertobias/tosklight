import type { PlaybackActionRequest } from "../../api/types";
import {
	type PoolPlaybackAction,
	type PoolPlaybackInput,
	poolPlaybackRequest,
} from "../server/playbackActionMapping";
import {
	assertGroupMaster,
	assertPlaybackPage,
	assertPlaybackPageOutcome,
	cueListReleaseRequest,
	groupActionRequest,
	isPlaybackSafetyRelease,
	isRetryablePlaybackFailure,
	playbackActionError,
} from "./actionWriterSupport";
import type { PlaybackIdentity, PlaybackOutcome } from "./contracts";
import { groupIdentity, identityKey, playbackIdentity } from "./contracts";
import type { PlaybackRuntimeStore } from "./store";

export type PlaybackRuntimeActionApply = (
	showId: string,
	deskId: string,
	request: PlaybackActionRequest,
) => Promise<PlaybackOutcome>;

export interface PlaybackDeskPageOutcome {
	desk_id: string;
	page: number;
	event_sequence: number | null;
	page_creation_event_sequence: number | null;
}

export type PlaybackDeskPageApply = (
	deskId: string,
	page: number,
) => Promise<PlaybackDeskPageOutcome>;

type CueListPlaybackIdentity = Extract<
	PlaybackIdentity,
	{ kind: "playback" | "cue_list" }
>;

export interface CueListRuntimeSource {
	identity: CueListPlaybackIdentity;
	cueListId: string;
}

export interface PlaybackRuntimeActions {
	setActivePage(page: number): Promise<boolean>;
	poolPlaybackAction(
		playbackNumber: number,
		action: PoolPlaybackAction,
		input?: PoolPlaybackInput,
	): Promise<PlaybackOutcome | null>;
	releaseCueListSource(
		source: CueListRuntimeSource,
	): Promise<PlaybackOutcome | null>;
	setGroupMaster(
		groupId: string,
		value: number,
	): Promise<PlaybackOutcome | null>;
	setGroupFlash(
		groupId: string,
		pressed: boolean,
	): Promise<PlaybackOutcome | null>;
}

interface PlaybackRuntimeActionWriterOptions {
	showId: string;
	deskId: string;
	store: PlaybackRuntimeStore;
	applyAction: PlaybackRuntimeActionApply;
	applyDeskPage?: PlaybackDeskPageApply;
	onError?: (error: Error | null) => void;
}

export class PlaybackRuntimeActionWriter implements PlaybackRuntimeActions {
	private stopped = false;

	constructor(private readonly options: PlaybackRuntimeActionWriterOptions) {}

	stop() {
		this.stopped = true;
	}

	async setActivePage(page: number) {
		try {
			assertPlaybackPage(page);
			return await this.setActivePageNow(page);
		} catch (reason) {
			this.rejectSetup(reason);
			return false;
		}
	}

	async poolPlaybackAction(
		playbackNumber: number,
		action: PoolPlaybackAction,
		input: PoolPlaybackInput = {},
	) {
		try {
			const request = poolPlaybackRequest(playbackNumber, action, input);
			if (
				isPlaybackSafetyRelease(action, input.pressed) &&
				!this.matchesScope()
			)
				return this.sendSafetyRelease(request);
			return await this.execute(playbackNumber, action, input, request);
		} catch (reason) {
			return this.rejectSetup(reason);
		}
	}

	async releaseCueListSource(source: CueListRuntimeSource) {
		try {
			return await this.executeCueListRelease(source);
		} catch (reason) {
			return this.rejectSetup(reason);
		}
	}

	async setGroupMaster(groupId: string, value: number) {
		try {
			assertGroupMaster(value);
			return await this.executeGroup(
				groupId,
				groupActionRequest(groupId, { type: "master", value }),
				value,
			);
		} catch (reason) {
			return this.rejectSetup(reason);
		}
	}

	async setGroupFlash(groupId: string, pressed: boolean) {
		try {
			const request = groupActionRequest(groupId, { type: "flash", pressed });
			if (!pressed && !this.matchesScope())
				return this.sendSafetyRelease(request);
			return await this.executeGroup(groupId, request);
		} catch (reason) {
			return this.rejectSetup(reason);
		}
	}

	private async setActivePageNow(page: number) {
		if (!this.options.applyDeskPage)
			throw new Error("Playback desk page actions are unavailable");
		const scope = this.options.store.captureScope();
		if (!this.isCurrent(scope)) return false;
		const token = this.options.store.beginOptimisticPage(page);
		if (!token) throw new Error("Authoritative Playback desk is loading");
		try {
			const outcome = await this.options.applyDeskPage(
				this.options.deskId,
				page,
			);
			return this.acceptPage(outcome, page, token, scope);
		} catch (reason) {
			return this.rollbackPage(reason, token, scope);
		}
	}

	private acceptPage(
		outcome: PlaybackDeskPageOutcome,
		page: number,
		token: string,
		scope: number,
	) {
		if (!this.isCurrent(scope)) return false;
		assertPlaybackPageOutcome(outcome, this.options.deskId, page);
		this.options.store.commitPage(token, page, outcome.event_sequence);
		this.reportActionError(null);
		return true;
	}

	private rollbackPage(reason: unknown, token: string, scope: number) {
		if (!this.isCurrent(scope)) return false;
		const error = playbackActionError(reason);
		if (this.options.store.rollbackPage(token, error))
			this.reportActionError(error);
		return false;
	}

	private async execute(
		playbackNumber: number,
		action: PoolPlaybackAction,
		input: PoolPlaybackInput,
		request: PlaybackActionRequest,
	) {
		const scope = this.options.store.captureScope();
		if (!this.isCurrent(scope)) return null;
		const optimistic = this.optimisticToken(playbackNumber, action, input);
		const token =
			optimistic ??
			this.options.store.beginRequest(playbackIdentity(playbackNumber));
		try {
			const outcome = await this.applyWithRetry(request, scope);
			return this.accept(outcome, request, token, scope);
		} catch (reason) {
			return this.rollback(reason, token, scope);
		}
	}

	private async executeCueListRelease(source: CueListRuntimeSource) {
		const scope = this.options.store.captureScope();
		if (!this.isCurrent(scope)) return null;
		this.assertRunningCueListSource(source);
		const request = cueListReleaseRequest(source);
		const token = this.options.store.beginRequest(source.identity);
		try {
			const outcome = await this.applyWithRetry(request, scope);
			return this.accept(outcome, request, token, scope);
		} catch (reason) {
			return this.rollback(reason, token, scope);
		}
	}

	private async executeGroup(
		groupId: string,
		request: PlaybackActionRequest,
		optimisticMaster?: number,
	) {
		const scope = this.options.store.captureScope();
		if (!this.isCurrent(scope)) return null;
		this.assertGroupAuthority(groupId);
		const identity = groupIdentity(groupId);
		const token =
			optimisticMaster == null
				? this.options.store.beginRequest(identity)
				: this.options.store.beginOptimisticMaster(identity, optimisticMaster);
		if (!token) throw new Error("Authoritative Group runtime is not ready");
		try {
			const outcome = await this.applyWithRetry(request, scope);
			assertGroupOutcome(outcome, groupId);
			return this.accept(outcome, request, token, scope);
		} catch (reason) {
			return this.rollback(reason, token, scope);
		}
	}

	private assertGroupAuthority(groupId: string) {
		const projection = this.options.store
			.getSnapshot()
			.projections.get(identityKey(groupIdentity(groupId)))
			?.find(
				(candidate) =>
					candidate.target === "group" && candidate.group_id === groupId,
			);
		if (!projection)
			throw new Error("Authoritative Group runtime is not ready");
	}

	private assertRunningCueListSource(source: CueListRuntimeSource) {
		const expectedPlayback =
			source.identity.kind === "playback"
				? source.identity.playback_number
				: null;
		if (
			source.identity.kind === "cue_list" &&
			source.identity.cue_list_id !== source.cueListId
		)
			throw new Error("Cuelist release identity does not match its source");
		const projections = this.options.store
			.getSnapshot()
			.projections.get(identityKey(source.identity));
		const running = projections?.some(
			(projection) =>
				projection.playback_number === expectedPlayback &&
				projection.target === "cue_list" &&
				projection.cue_list_id === source.cueListId &&
				projection.runtime !== null,
		);
		if (!running) throw new Error("Authoritative Cuelist runtime is not ready");
	}

	private optimisticToken(
		playbackNumber: number,
		action: PoolPlaybackAction,
		input: PoolPlaybackInput,
	) {
		return action === "master" && input.value != null
			? this.options.store.beginOptimisticMaster(playbackNumber, input.value)
			: null;
	}

	private async applyWithRetry(request: PlaybackActionRequest, scope: number) {
		try {
			return await this.options.applyAction(
				this.options.showId,
				this.options.deskId,
				request,
			);
		} catch (reason) {
			if (!this.isCurrent(scope) || !isRetryablePlaybackFailure(reason))
				throw reason;
			return this.options.applyAction(
				this.options.showId,
				this.options.deskId,
				request,
			);
		}
	}

	private async sendSafetyRelease(request: PlaybackActionRequest) {
		try {
			return await this.options.applyAction(
				this.options.showId,
				this.options.deskId,
				request,
			);
		} catch (reason) {
			if (!isRetryablePlaybackFailure(reason)) throw reason;
			return this.options.applyAction(
				this.options.showId,
				this.options.deskId,
				request,
			);
		}
	}

	private accept(
		outcome: PlaybackOutcome,
		request: PlaybackActionRequest,
		token: string,
		scope: number,
	) {
		if (!this.isCurrent(scope)) return null;
		if (outcome.request_id !== request.request_id)
			throw new Error("Playback outcome request ID does not match the request");
		if (!this.options.store.installOutcome(outcome, token)) return null;
		this.reportActionError(null);
		return outcome;
	}

	private rollback(reason: unknown, token: string, scope: number) {
		if (!this.isCurrent(scope)) return null;
		const error = playbackActionError(reason);
		if (!this.options.store.rollbackProjection(token, error)) return null;
		this.reportActionError(error);
		return null;
	}

	private rejectSetup(reason: unknown) {
		const error = playbackActionError(reason);
		if (!this.matchesScope()) return null;
		this.options.store.reportActionError(error);
		this.reportActionError(error);
		return null;
	}

	private reportActionError(error: Error | null) {
		if (this.options.store.getSnapshot().status !== "error")
			this.options.onError?.(error);
	}

	private isCurrent(scope: number) {
		return this.options.store.isScopeCurrent(scope) && this.matchesScope();
	}

	private matchesScope() {
		const state = this.options.store.getSnapshot();
		return (
			!this.stopped &&
			state.showId === this.options.showId &&
			state.deskId === this.options.deskId
		);
	}
}

function assertGroupOutcome(outcome: PlaybackOutcome, groupId: string) {
	if (
		outcome.requested.kind !== "group" ||
		outcome.requested.group_id !== groupId ||
		outcome.resolved.kind !== "group" ||
		outcome.resolved.group_id !== groupId ||
		outcome.projection.target !== "group" ||
		outcome.projection.group_id !== groupId ||
		outcome.projection.requested.kind !== "group" ||
		outcome.projection.requested.group_id !== groupId ||
		outcome.projection.playback_number !== outcome.resolved.playback_number
	)
		throw new Error("Playback outcome does not match the Group request");
}
