type Operation = () => Promise<void>;
type PendingPicture = { operation: Operation; result: Promise<void> };
type Owner = { tail: Promise<void>; count: number; picture?: PendingPicture; closing?: Promise<void> };

/** Only short native acknowledgements, never image jobs. Exact input/capture FIFO per pane;
 * adjacent pending full Pictures may replace one another. At most 128 ordinary operations,
 * one reserved latest Picture and one idempotent close, so backpressure cannot leak a pane. */
export class StagePaneCommandQueue {
	private readonly pending = new Map<string, Owner>();

	run(paneId: string, operation: Operation): Promise<void> {
		const owner = this.owner(paneId);
		if (owner.closing) return Promise.reject(new Error("Stage pane is closing"));
		if (owner.count >= 128) {
			return Promise.reject(new Error("Stage command queue is full; wait for the pane to respond"));
		}
		owner.picture = undefined; // Never coalesce a Picture across an input/capture barrier.
		return this.append(paneId, owner, operation);
	}

	picture(paneId: string, operation: Operation): Promise<void> {
		const owner = this.owner(paneId);
		if (owner.closing) return owner.closing; // Retired picture; reopen replays current state.
		if (owner.picture) {
			owner.picture.operation = operation;
			return owner.picture.result;
		}
		const entry: PendingPicture = { operation, result: Promise.resolve() };
		entry.result = this.append(paneId, owner, () => {
			if (owner.picture === entry) owner.picture = undefined;
			return entry.operation();
		});
		owner.picture = entry;
		return entry.result;
	}

	close(paneId: string, operation: Operation): Promise<void> {
		const owner = this.owner(paneId);
		if (owner.closing) return owner.closing;
		owner.picture = undefined;
		owner.closing = this.append(paneId, owner, operation);
		return owner.closing;
	}

	private owner(paneId: string): Owner {
		let owner = this.pending.get(paneId);
		if (!owner) {
			owner = { tail: Promise.resolve(), count: 0 };
			this.pending.set(paneId, owner);
		}
		return owner;
	}

	private append(paneId: string, owner: Owner, operation: Operation): Promise<void> {
		const result = owner.tail.catch(() => undefined).then(operation);
		owner.tail = result;
		owner.count++;
		const retire = () => {
			owner.count--;
			if (owner.count === 0 && this.pending.get(paneId) === owner) this.pending.delete(paneId);
		};
		void result.then(retire, retire);
		return result;
	}
}
