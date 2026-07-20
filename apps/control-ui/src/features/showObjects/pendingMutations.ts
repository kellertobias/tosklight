import type {
	ShowObjectBodies,
	ShowObjectKind,
} from "./contracts";
import type { PendingMutation } from "./storeTypes";

export class ShowObjectPendingMutations {
	private readonly operations = new Map<string, PendingMutation[]>();

	clear() {
		this.operations.clear();
	}

	values() {
		return this.operations.values();
	}

	keys() {
		return this.operations.keys();
	}

	begin<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
		body: ShowObjectBodies[K] | null,
		baseEventSequence: number,
	) {
		const token = crypto.randomUUID();
		const key = `${kind}:${objectId}`;
		const current = this.operations.get(key) ?? [];
		current.push({ token, showId, kind, objectId, body, baseEventSequence });
		this.operations.set(key, current);
		return token;
	}

	take(token: string): PendingMutation | null {
		for (const [key, current] of this.operations) {
			const index = current.findIndex((operation) => operation.token === token);
			if (index < 0) continue;
			const [operation] = current.splice(index, 1);
			if (!current.length) this.operations.delete(key);
			return operation;
		}
		return null;
	}
}
