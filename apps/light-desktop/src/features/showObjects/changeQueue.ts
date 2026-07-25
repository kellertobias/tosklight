import type { ShowObjectsChange } from "./contracts";

/** Sequence-ordered, transaction-preserving buffer that merges replay duplicates. */
export class ShowObjectsChangeQueue {
	private readonly changes = new Map<number, ShowObjectsChange>();

	get size() {
		return this.changes.size;
	}

	clear() {
		this.changes.clear();
	}

	push(change: ShowObjectsChange) {
		const existing = this.changes.get(change.eventSequence);
		if (!existing) {
			this.changes.set(change.eventSequence, change);
			return;
		}
		const objects = new Map(
			existing.changes.map((item) => [`${item.kind}\0${item.objectId}`, item]),
		);
		for (const item of change.changes)
			objects.set(`${item.kind}\0${item.objectId}`, item);
		this.changes.set(change.eventSequence, {
			...change,
			showRevision: Math.max(existing.showRevision, change.showRevision),
			changes: [...objects.values()],
		});
	}

	shift() {
		if (!this.changes.size) return null;
		const sequence = Math.min(...this.changes.keys());
		const change = this.changes.get(sequence) ?? null;
		this.changes.delete(sequence);
		return change;
	}
}
