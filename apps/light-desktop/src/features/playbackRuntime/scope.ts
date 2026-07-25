import type { PlaybackIdentity, PlaybackProjection } from "./contracts";
import { identityKey, projectionKeys } from "./contracts";
import type { PlaybackEventScope } from "./transport";

export class PlaybackViewScope {
	private readonly identities = new Map<
		string,
		{ identity: PlaybackIdentity; references: number }
	>();
	private deskReferences = 0;

	activate(identity: PlaybackIdentity) {
		const key = identityKey(identity);
		const current = this.identities.get(key);
		this.identities.set(key, {
			identity,
			references: (current?.references ?? 0) + 1,
		});
		return current == null;
	}

	deactivate(identity: PlaybackIdentity) {
		const key = identityKey(identity);
		const current = this.identities.get(key);
		if (!current) return false;
		if (current.references > 1)
			this.identities.set(key, {
				...current,
				references: current.references - 1,
			});
		else this.identities.delete(key);
		return current.references === 1;
	}

	activateDesk() {
		this.deskReferences++;
		return this.deskReferences === 1;
	}

	deactivateDesk() {
		const hadLastReference = this.deskReferences === 1;
		this.deskReferences = Math.max(0, this.deskReferences - 1);
		return hadLastReference;
	}

	includesProjection(projection: PlaybackProjection) {
		return projectionKeys(projection).some((key) => this.identities.has(key));
	}

	subscription(): PlaybackEventScope {
		return { identities: this.values(), desk: this.deskReferences > 0 };
	}

	values() {
		return [...this.identities.values()]
			.map(({ identity }) => identity)
			.sort((left, right) =>
				identityKey(left).localeCompare(identityKey(right)),
			);
	}

	hasViews() {
		return this.identities.size > 0 || this.deskReferences > 0;
	}

	hasIdentities() {
		return this.identities.size > 0;
	}

	clear() {
		this.identities.clear();
		this.deskReferences = 0;
	}

	key() {
		return JSON.stringify({
			identities: this.values().map(identityKey),
			desk: this.deskReferences > 0,
		});
	}
}
