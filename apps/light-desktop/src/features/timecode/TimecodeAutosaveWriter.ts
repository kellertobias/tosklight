import { ApiRequestError } from "../../api/ApiRequestError";
import type { ShowObjectActionOutcome } from "../../api/generated/light-wire";
import type {
	TimecodeCollectionSnapshot,
	TimecodeDefinition,
	TimecodeObjectRecord,
	TimecodePatch,
} from "../../api/types/timecode";

export interface TimecodeAutosaveApi {
	create(
		showId: string,
		definition: TimecodeDefinition,
	): Promise<ShowObjectActionOutcome>;
	update(
		showId: string,
		timecodeId: string,
		expectedRevision: number,
		patch: TimecodePatch,
	): Promise<ShowObjectActionOutcome>;
	objects(showId: string): Promise<TimecodeCollectionSnapshot>;
}

interface Waiter {
	version: number;
	resolve(record: TimecodeObjectRecord): void;
	reject(reason: unknown): void;
}

/**
 * Serializes immediate Timecode edits and coalesces changes which have not yet
 * reached the server. Every update therefore uses the revision returned by the
 * preceding authoritative mutation rather than the revision captured when the
 * editor opened.
 */
export class TimecodeAutosaveWriter {
	private authority: TimecodeObjectRecord | null;
	private desired: TimecodeDefinition;
	private requestedVersion = 0;
	private settledVersion = 0;
	private running: Promise<void> | null = null;
	private waiters: Waiter[] = [];
	private lastError: unknown = null;

	constructor(
		private readonly showId: string,
		initial: TimecodeObjectRecord | null,
		private readonly api: TimecodeAutosaveApi,
	) {
		this.authority = initial ? cloneRecord(initial) : null;
		this.desired = cloneDefinition(
			initial?.definition ?? {
				id: "",
				number: 0,
				name: "",
				transport_offset_frame: 0,
				auto_start: false,
				markers: [],
				lanes: [],
			},
		);
	}

	enqueue(definition: TimecodeDefinition): Promise<TimecodeObjectRecord> {
		this.lastError = null;
		this.desired = cloneDefinition(definition);
		const version = ++this.requestedVersion;
		const result = new Promise<TimecodeObjectRecord>((resolve, reject) => {
			this.waiters.push({ version, resolve, reject });
		});
		this.start();
		return result;
	}

	current(): TimecodeObjectRecord | null {
		return this.authority ? cloneRecord(this.authority) : null;
	}

	async flush(): Promise<TimecodeObjectRecord | null> {
		await this.running;
		if (this.lastError) throw this.lastError;
		return this.current();
	}

	private start() {
		if (this.running) return;
		this.running = this.pump().finally(() => {
			this.running = null;
			if (this.settledVersion < this.requestedVersion) this.start();
		});
	}

	private async pump() {
		try {
			while (this.settledVersion < this.requestedVersion) {
				const version = this.requestedVersion;
				const target = cloneDefinition(this.desired);
				this.authority = await this.persist(target);
				this.settledVersion = version;
				this.resolveThrough(version);
			}
		} catch (reason) {
			this.lastError = reason;
			const pending = this.waiters;
			this.waiters = [];
			for (const waiter of pending) waiter.reject(reason);
			this.settledVersion = this.requestedVersion;
		}
	}

	private async persist(
		target: TimecodeDefinition,
	): Promise<TimecodeObjectRecord> {
		if (!this.authority) {
			const outcome = await this.api.create(this.showId, target);
			return recordFromOutcome(outcome, target);
		}
		const patch = timecodePatch(this.authority.definition, target);
		if (!Object.keys(patch).length)
			return { revision: this.authority.revision, definition: target };
		try {
			const outcome = await this.api.update(
				this.showId,
				target.id,
				this.authority.revision,
				patch,
			);
			return recordFromOutcome(outcome, target);
		} catch (reason) {
			if (!(reason instanceof ApiRequestError) || reason.status !== 409)
				throw reason;
			const current = (await this.api.objects(this.showId)).objects.find(
				(record) => record.definition.id === target.id,
			);
			if (!current)
				throw new Error(
					`Timecode ${target.number} no longer exists; the latest edit was not saved.`,
				);
			this.authority = cloneRecord(current);
			const rebasedTarget: TimecodeDefinition = {
				...current.definition,
				...patch,
			};
			const outcome = await this.api.update(
				this.showId,
				target.id,
				current.revision,
				patch,
			);
			return recordFromOutcome(outcome, rebasedTarget);
		}
	}

	private resolveThrough(version: number) {
		if (!this.authority) return;
		const remaining: Waiter[] = [];
		for (const waiter of this.waiters) {
			if (waiter.version <= version)
				waiter.resolve(cloneRecord(this.authority));
			else remaining.push(waiter);
		}
		this.waiters = remaining;
	}
}

export function timecodePatch(
	previous: TimecodeDefinition,
	next: TimecodeDefinition,
): TimecodePatch {
	const patch: TimecodePatch = {};
	if (previous.number !== next.number) patch.number = next.number;
	if (previous.name !== next.name) patch.name = next.name;
	if (previous.duration_frame !== next.duration_frame)
		patch.duration_frame = next.duration_frame;
	if (previous.transport_offset_frame !== next.transport_offset_frame)
		patch.transport_offset_frame = next.transport_offset_frame;
	if (previous.auto_start !== next.auto_start)
		patch.auto_start = next.auto_start;
	if (!same(previous.audio, next.audio)) patch.audio = next.audio ?? null;
	if (!same(previous.markers, next.markers))
		patch.markers = cloneDefinition(next).markers;
	if (!same(previous.lanes, next.lanes))
		patch.lanes = cloneDefinition(next).lanes;
	return patch;
}

function recordFromOutcome(
	outcome: ShowObjectActionOutcome,
	target: TimecodeDefinition,
): TimecodeObjectRecord {
	return {
		revision: outcome.object.revision,
		definition: cloneDefinition(target),
	};
}

function same(left: unknown, right: unknown) {
	return JSON.stringify(left ?? null) === JSON.stringify(right ?? null);
}

function cloneRecord(record: TimecodeObjectRecord): TimecodeObjectRecord {
	return {
		revision: record.revision,
		definition: cloneDefinition(record.definition),
	};
}

function cloneDefinition(definition: TimecodeDefinition): TimecodeDefinition {
	return structuredClone(definition);
}
