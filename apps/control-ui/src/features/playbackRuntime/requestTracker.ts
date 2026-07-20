import type { PlaybackProjection } from "./contracts";
import { upsertProjection } from "./projectionCollection";

export interface PlaybackRequestContext {
	token: string;
	key: string;
	baseSequences: ReadonlyMap<string, number>;
	deskBaseSequence: number;
	optimistic: PlaybackProjection | null;
	order: number;
}

export class PlaybackRequestTracker {
	private readonly requests = new Map<string, PlaybackRequestContext>();
	private readonly latestOptimisticTokens = new Map<string, string>();
	private order = 0;

	reset() {
		this.requests.clear();
		this.latestOptimisticTokens.clear();
		this.order = 0;
	}

	begin(
		key: string,
		baseSequences: ReadonlyMap<string, number>,
		deskBaseSequence: number,
		optimistic: PlaybackProjection | null = null,
	) {
		const request = {
			token: crypto.randomUUID(),
			key,
			baseSequences,
			deskBaseSequence,
			optimistic,
			order: ++this.order,
		};
		this.requests.set(request.token, request);
		if (optimistic)
			this.latestOptimisticTokens.set(request.key, request.token);
		return request.token;
	}

	peek(token?: string | null) {
		return token ? (this.requests.get(token) ?? null) : null;
	}

	take(token?: string | null) {
		const request = this.peek(token);
		if (!request) return null;
		this.requests.delete(request.token);
		if (this.latestOptimisticTokens.get(request.key) === request.token)
			this.replaceLatestOptimistic(request.key);
		return request;
	}

	render(
		key: string,
		authoritative: readonly PlaybackProjection[],
	): readonly PlaybackProjection[] {
		const token = this.latestOptimisticTokens.get(key);
		const optimistic = token ? this.requests.get(token)?.optimistic : null;
		return optimistic
			? upsertProjection(authoritative, optimistic)
			: authoritative;
	}

	pendingKeys() {
		return new Set(
			[...this.requests.values()]
				.filter((request) => request.optimistic)
				.map((request) => request.key),
		);
	}

	private replaceLatestOptimistic(key: string) {
		const latest = [...this.requests.values()]
			.filter((request) => request.key === key && request.optimistic)
			.sort((left, right) => right.order - left.order)[0];
		if (latest) this.latestOptimisticTokens.set(key, latest.token);
		else this.latestOptimisticTokens.delete(key);
	}
}
