/** Separates installed event progress from authoritative snapshot coverage. */
export class ShowObjectsCursors {
	private applied: number | null = null;
	private snapshot: number | null = null;

	resume() {
		if (this.applied == null) return this.snapshot;
		if (this.snapshot == null) return this.applied;
		return Math.max(this.applied, this.snapshot);
	}

	installEvent(sequence: number) {
		this.applied = Math.max(this.applied ?? 0, sequence);
	}

	installSnapshot(sequence: number) {
		this.snapshot = Math.max(this.snapshot ?? 0, sequence);
	}

	reset() {
		this.applied = null;
		this.snapshot = null;
	}
}
