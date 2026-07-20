import type { ShowObjectKind } from "./contracts";
import { objectKey } from "./storeProjection";

/** Orders collection snapshots, exact responses, and retained events per object. */
export class ShowObjectEventWatermarks {
	private readonly objectSequences = new Map<string, number>();
	private readonly objectFloors = new Map<string, number>();
	private readonly kindFloors = new Map<ShowObjectKind, number>();

	clear() {
		this.objectSequences.clear();
		this.objectFloors.clear();
		this.kindFloors.clear();
	}

	setKindFloor(kind: ShowObjectKind, sequence: number) {
		this.kindFloors.set(kind, sequence);
	}

	objectSequence(key: string) {
		return this.objectSequences.get(key);
	}

	objectFloor(key: string) {
		return this.objectFloors.get(key);
	}

	appliedSequence(kind: ShowObjectKind, key: string) {
		return Math.max(
			this.kindFloors.get(kind) ?? 0,
			this.objectSequences.get(key) ?? 0,
		);
	}

	hasAppliedAtOrAfter(
		kind: ShowObjectKind,
		key: string,
		minimum?: number | null,
	) {
		if (minimum == null) return false;
		return (
			(this.kindFloors.get(kind) ?? -1) >= minimum ||
			(this.objectSequences.get(key) ?? -1) >= minimum
		);
	}

	raiseObjectFloor(
		kind: ShowObjectKind,
		objectId: string,
		minimum?: number | null,
	) {
		if (minimum == null) return;
		const key = objectKey(kind, objectId);
		if (minimum <= this.appliedSequence(kind, key)) return;
		this.objectFloors.set(
			key,
			Math.max(this.objectFloors.get(key) ?? 0, minimum),
		);
	}

	sealExactResponse(
		kind: ShowObjectKind,
		objectId: string,
		minimum?: number | null,
	) {
		if (minimum == null) return;
		const key = objectKey(kind, objectId);
		this.objectSequences.set(
			key,
			Math.max(this.objectSequences.get(key) ?? 0, minimum),
		);
		if ((this.objectFloors.get(key) ?? 0) <= minimum)
			this.objectFloors.delete(key);
	}

	acceptChange(kind: ShowObjectKind, objectId: string, sequence: number) {
		const key = objectKey(kind, objectId);
		const kindFloor = this.kindFloors.get(kind) ?? 0;
		const objectFloor = this.objectFloors.get(key) ?? 0;
		const applied = this.objectSequences.get(key) ?? 0;
		if (sequence <= kindFloor || sequence < objectFloor || sequence <= applied)
			return false;
		this.objectSequences.set(key, sequence);
		if (sequence >= objectFloor) this.objectFloors.delete(key);
		return true;
	}

	clearKind(kind: ShowObjectKind, eventFloor?: number) {
		this.clearMatching(this.objectSequences, kind, eventFloor);
		this.clearMatching(this.objectFloors, kind, eventFloor);
		this.kindFloors.delete(kind);
	}

	private clearMatching(
		values: Map<string, number>,
		kind: ShowObjectKind,
		eventFloor?: number,
	) {
		const prefix = `${kind}:`;
		for (const [key, sequence] of values)
			if (
				key.startsWith(prefix) &&
				(eventFloor == null || sequence <= eventFloor)
			)
				values.delete(key);
	}
}
