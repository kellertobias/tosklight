import type { PlaybackActionRequest } from "../../api/types";
import {
	poolPlaybackRequest,
	type PoolPlaybackAction,
	type PoolPlaybackInput,
} from "../server/playbackActionMapping";
import type { PlaybackOutcome } from "./contracts";
import { playbackIdentity } from "./contracts";
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

export interface PlaybackRuntimeActions {
	setActivePage(page: number): Promise<boolean>;
	poolPlaybackAction(
		playbackNumber: number,
		action: PoolPlaybackAction,
		input?: PoolPlaybackInput,
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
			assertPage(page);
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
			if (isSafetyRelease(action, input) && !this.matchesScope())
				return this.sendSafetyRelease(request);
			return await this.execute(playbackNumber, action, input, request);
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
			const outcome = await this.options.applyDeskPage(this.options.deskId, page);
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
		assertPageOutcome(outcome, this.options.deskId, page);
		this.options.store.commitPage(token, page, outcome.event_sequence);
		this.reportActionError(null);
		return true;
	}

	private rollbackPage(reason: unknown, token: string, scope: number) {
		if (!this.isCurrent(scope)) return false;
		const error = asError(reason);
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

	private optimisticToken(
		playbackNumber: number,
		action: PoolPlaybackAction,
		input: PoolPlaybackInput,
	) {
		return action === "master" && input.value != null
			? this.options.store.beginOptimisticMaster(playbackNumber, input.value)
			: null;
	}

	private async applyWithRetry(
		request: PlaybackActionRequest,
		scope: number,
	) {
		try {
			return await this.options.applyAction(
				this.options.showId,
				this.options.deskId,
				request,
			);
		} catch (reason) {
			if (!this.isCurrent(scope) || !isRetryable(reason)) throw reason;
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
			if (!isRetryable(reason)) throw reason;
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

	private rollback(
		reason: unknown,
		token: string,
		scope: number,
	) {
		if (!this.isCurrent(scope)) return null;
		const error = asError(reason);
		if (!this.options.store.rollbackProjection(token, error)) return null;
		this.reportActionError(error);
		return null;
	}

	private rejectSetup(reason: unknown) {
		const error = asError(reason);
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

function isSafetyRelease(
	action: PoolPlaybackAction,
	input: PoolPlaybackInput,
) {
	return (
		input.pressed === false &&
		(action === "button" || action === "flash" || action === "swap")
	);
}

function isRetryable(reason: unknown) {
	if (reason instanceof TypeError) return true;
	if (!(reason instanceof Error)) return false;
	const failure = reason as Error & { retryable?: unknown; status?: unknown };
	if (typeof failure.retryable === "boolean") return failure.retryable;
	return (
		failure.status === 0 ||
		failure.status === 408 ||
		failure.status === 429 ||
		(typeof failure.status === "number" && failure.status >= 500)
	);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}

function assertPage(page: number) {
	if (!Number.isSafeInteger(page) || page < 1 || page > 127)
		throw new Error("Playback page must be an integer between 1 and 127");
}

function assertPageOutcome(
	outcome: PlaybackDeskPageOutcome,
	deskId: string,
	page: number,
) {
	if (outcome.desk_id !== deskId || outcome.page !== page)
		throw new Error("Playback page response does not match the active desk request");
	assertOptionalSequence(outcome.event_sequence, "event sequence");
	assertOptionalSequence(
		outcome.page_creation_event_sequence,
		"page creation event sequence",
	);
	if (outcome.page_creation_event_sequence !== null)
		throw new Error("Playback page selection unexpectedly created a Page");
}

function assertOptionalSequence(value: number | null, label: string) {
	if (value != null && (!Number.isSafeInteger(value) || value < 0))
		throw new Error(`Playback page response ${label} is invalid`);
}
