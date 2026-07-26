import type { PlaybackIdentity, PlaybackProjection } from "./contracts";
import { identityKey, projectionKeys } from "./contracts";
import type { PlaybackEventScope } from "./transport";

export class PlaybackViewScope {
	private readonly identities = new Map<
		string,
		{
			identity: PlaybackIdentity;
			references: number;
			telemetryReferences: number;
		}
	>();
	private deskReferences = 0;
	private deskTelemetryReferences = 0;

	activate(identity: PlaybackIdentity, telemetry = true) {
		const previousKey = this.key();
		const key = identityKey(identity);
		const current = this.identities.get(key);
		this.identities.set(key, {
			identity,
			references: (current?.references ?? 0) + 1,
			telemetryReferences:
				(current?.telemetryReferences ?? 0) + (telemetry ? 1 : 0),
		});
		return previousKey !== this.key();
	}

	deactivate(identity: PlaybackIdentity, telemetry = true) {
		const previousKey = this.key();
		const key = identityKey(identity);
		const current = this.identities.get(key);
		if (!current) return false;
		if (current.references > 1)
			this.identities.set(key, {
				...current,
				references: current.references - 1,
				telemetryReferences: Math.max(
					0,
					current.telemetryReferences - (telemetry ? 1 : 0),
				),
			});
		else this.identities.delete(key);
		return previousKey !== this.key();
	}

	activateDesk(telemetry = true) {
		const previousKey = this.key();
		this.deskReferences++;
		if (telemetry) this.deskTelemetryReferences++;
		return previousKey !== this.key();
	}

	deactivateDesk(telemetry = true) {
		const previousKey = this.key();
		this.deskReferences = Math.max(0, this.deskReferences - 1);
		if (telemetry)
			this.deskTelemetryReferences = Math.max(
				0,
				this.deskTelemetryReferences - 1,
			);
		return previousKey !== this.key();
	}

	includesProjection(projection: PlaybackProjection) {
		return projectionKeys(projection).some((key) => this.identities.has(key));
	}

	subscription(): PlaybackEventScope {
		const scope: PlaybackEventScope = {
			identities: this.values(),
			desk: this.deskReferences > 0,
		};
		if (!this.includesTelemetry()) scope.telemetry = false;
		return scope;
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
		this.deskTelemetryReferences = 0;
	}

	key() {
		return JSON.stringify({
			identities: this.values().map(identityKey),
			desk: this.deskReferences > 0,
			telemetry: this.includesTelemetry(),
		});
	}

	private includesTelemetry() {
		return (
			this.deskTelemetryReferences > 0 ||
			[...this.identities.values()].some(
				({ telemetryReferences }) => telemetryReferences > 0,
			)
		);
	}
}
