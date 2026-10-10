import { formatErrorDetails } from "@tosklight/ui";
import {
	createContext,
	type RefObject,
	type PropsWithChildren,
	useContext,
	useMemo,
	useRef,
	useState,
} from "react";
import type {
	VirtualPlaybackZone,
	VirtualPlaybackZonesAuthority,
	VirtualPlaybackZonesCapability,
	VirtualPlaybackZonesEventStream,
	VirtualPlaybackZonesScope,
	VirtualPlaybackZonesSnapshot,
	VirtualPlaybackZonesTransport,
} from "./contracts";

interface VirtualPlaybackZonesProviderProps {
	readonly authority: VirtualPlaybackZonesAuthority | null;
	readonly transport: VirtualPlaybackZonesTransport | null;
}

interface AuthorityEpoch {
	readonly authorityId: string | null;
	readonly showId: string | null;
	readonly transport: VirtualPlaybackZonesTransport | null;
	readonly generation: number;
}

interface ReportedError {
	readonly generation: number;
	readonly message: string;
}

const VirtualPlaybackZonesContext =
	createContext<VirtualPlaybackZonesCapability | null>(null);

/** Explicit action/query boundary; mounting it performs no reads. */
export function VirtualPlaybackZonesProvider({
	authority,
	transport,
	children,
}: PropsWithChildren<VirtualPlaybackZonesProviderProps>) {
	const epochRef = useRef<AuthorityEpoch>(initialEpoch());
	const [reportedError, setReportedError] = useState<ReportedError | null>(
		null,
	);
	const epoch = updateEpoch(epochRef, authority, transport);
	const controller = useMemo(
		() =>
			authority && transport
				? new VirtualPlaybackZonesController(
						authority.scope,
						transport,
						() => epochRef.current.generation === epoch.generation,
						(error) =>
							setReportedError(
								error
									? {
											generation: epoch.generation,
											message: formatErrorDetails(error),
										}
									: null,
							),
					)
				: null,
		[epoch.generation],
	);
	const error =
		reportedError?.generation === epoch.generation
			? reportedError.message
			: null;
	const capability = useMemo<VirtualPlaybackZonesCapability>(
		() => ({
			authorityId: authority?.authorityId ?? null,
			authorityGeneration: epoch.generation,
			available: controller !== null,
			error,
			getZones: () => controller?.getZones() ?? null,
			isSaving: () => controller?.isSaving() ?? false,
			subscribe: (listener) => controller?.subscribe(listener) ?? noOp,
			activate: () => controller?.activate() ?? noOp,
			load: () => controller?.load() ?? Promise.resolve(null),
			save: (zones) => controller?.save(zones) ?? Promise.resolve(null),
			clearError: () => setReportedError(null),
		}),
		[authority?.authorityId, controller, epoch.generation, error],
	);
	return (
		<VirtualPlaybackZonesContext.Provider value={capability}>
			{children}
		</VirtualPlaybackZonesContext.Provider>
	);
}

export function useVirtualPlaybackZones() {
	const capability = useContext(VirtualPlaybackZonesContext);
	if (!capability)
		throw new Error("VirtualPlaybackZonesProvider is not mounted");
	return capability;
}

export class VirtualPlaybackZonesController {
	private pendingSnapshot: Promise<VirtualPlaybackZonesSnapshot | null> | null =
		null;
	private saveTail: Promise<void> = Promise.resolve();
	private snapshot: VirtualPlaybackZonesSnapshot | null = null;
	private readonly listeners = new Set<() => void>();
	private eventStream: VirtualPlaybackZonesEventStream | null = null;
	private activeCount = 0;
	private saveCount = 0;
	private mutationVersion = 0;
	private eventGeneration = 0;
	private reconnectAttempt = 0;
	private reconnectTimer: ReturnType<typeof globalThis.setTimeout> | null =
		null;
	private eventFailure: Error | null = null;

	constructor(
		private readonly scope: VirtualPlaybackZonesScope,
		private readonly transport: VirtualPlaybackZonesTransport,
		private readonly isCurrent: () => boolean,
		private readonly reportError: (error: Error | null) => void,
	) {}

	load() {
		const cached = this.getZones();
		return cached
			? Promise.resolve(cached)
			: this.loadSnapshot().then((snapshot) => snapshot?.zones ?? null);
	}

	getZones() {
		if (!this.isCurrent()) return null;
		return this.snapshot?.zones ?? null;
	}

	isSaving() {
		return this.isCurrent() && this.saveCount > 0;
	}

	subscribe(listener: () => void) {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	}

	activate() {
		this.activeCount += 1;
		if (this.activeCount === 1) this.openWindow();
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			this.activeCount = Math.max(0, this.activeCount - 1);
			if (this.activeCount === 0) {
				this.eventGeneration += 1;
				if (this.reconnectTimer !== null)
					globalThis.clearTimeout(this.reconnectTimer);
				this.reconnectTimer = null;
				this.eventStream?.close();
				this.eventStream = null;
			}
		};
	}

	private openWindow() {
		this.snapshot = null;
		this.reconnectAttempt = 0;
		this.notify();
		this.openEvents();
	}

	private openEvents() {
		const generation = ++this.eventGeneration;
		const current = () =>
			this.isCurrent() &&
			this.activeCount > 0 &&
			generation === this.eventGeneration;
		try {
			const stream =
				this.transport.subscribe?.(this.scope, {
					ready: () => {
						if (!current()) return;
						void this.reloadSnapshot(current).then((snapshot) => {
							if (!snapshot || !current()) return;
							this.reconnectAttempt = 0;
							this.eventFailure = null;
							this.reportError(null);
						});
					},
					changed: (change) => {
						if (
							current() &&
							change.showId === this.scope.showId &&
							change.revision !== this.snapshot?.revision
						)
							void this.reloadSnapshot(current);
					},
					gap: () => {
						if (current()) void this.reloadSnapshot(current);
					},
					error: (error) => {
						if (current()) this.reconnect(error);
					},
					closed: () => {
						if (current())
							this.reconnect(
								new Error("Virtual Playback zone event connection closed"),
							);
					},
				}) ?? null;
			if (current()) this.eventStream = stream;
			else stream?.close();
		} catch (reason) {
			if (current()) this.reconnect(asError(reason));
		}
	}

	private reconnect(error: Error) {
		this.eventFailure = error;
		this.reportError(error);
		this.eventGeneration += 1;
		this.eventStream?.close();
		this.eventStream = null;
		if (this.reconnectTimer !== null) return;
		const delays = [250, 500, 1000, 2000, 4000];
		const delay = delays[this.reconnectAttempt++];
		if (delay === undefined) {
			this.eventFailure = new Error(
				"Virtual Playback zone connection could not be restored. Reopen the pane to retry.",
			);
			this.reportError(this.eventFailure);
			return;
		}
		this.reconnectTimer = globalThis.setTimeout(() => {
			this.reconnectTimer = null;
			if (this.isCurrent() && this.activeCount > 0) this.openEvents();
		}, delay);
	}

	save(zones: readonly VirtualPlaybackZone[]) {
		this.saveCount += 1;
		this.notify();
		return this.enqueueSave(() => this.performSave(zones)).finally(() => {
			this.saveCount = Math.max(0, this.saveCount - 1);
			if (this.isCurrent()) this.notify();
		});
	}

	private async performSave(zones: readonly VirtualPlaybackZone[]) {
		try {
			if (!this.snapshot && !(await this.loadSnapshot())) return null;
			const outcome = await this.transport.save(
				this.scope,
				this.snapshot?.revision ?? 0,
				zones,
				crypto.randomUUID(),
			);
			if (!this.isCurrent()) return null;
			this.mutationVersion += 1;
			this.storeSnapshot(outcome);
			this.reportError(this.eventFailure);
			return outcome.zones;
		} catch (reason) {
			if (conflictStatus(reason) === 409) await this.reloadSnapshot();
			return this.failure(reason);
		}
	}

	private enqueueSave(
		operation: () => Promise<readonly VirtualPlaybackZone[] | null>,
	) {
		const result = this.saveTail.then(operation, operation);
		this.saveTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	private reloadSnapshot(accept?: () => boolean) {
		this.pendingSnapshot = null;
		return this.loadSnapshot(accept);
	}

	private loadSnapshot(accept?: () => boolean) {
		if (this.pendingSnapshot) return this.pendingSnapshot;
		const load = this.performLoad(accept);
		this.pendingSnapshot = load;
		void load.finally(() => {
			if (this.pendingSnapshot === load) this.pendingSnapshot = null;
		});
		return load;
	}

	private async performLoad(accept?: () => boolean) {
		const loadVersion = this.mutationVersion;
		try {
			const snapshot = await this.transport.loadSnapshot(this.scope);
			if (!this.isCurrent() || (accept && !accept())) return null;
			if (snapshot.showId !== this.scope.showId)
				throw new Error(
					"Virtual Playback zone response changed authority scope",
				);
			if (loadVersion === this.mutationVersion) this.storeSnapshot(snapshot);
			this.reportError(this.eventFailure);
			return snapshot;
		} catch (reason) {
			if (accept && !accept()) return null;
			return this.failure(reason);
		}
	}

	private storeSnapshot(snapshot: VirtualPlaybackZonesSnapshot) {
		if (this.snapshot && sameSnapshot(this.snapshot, snapshot)) return;
		this.snapshot = snapshot;
		this.notify();
	}

	private notify() {
		for (const listener of this.listeners) listener();
	}

	private failure(reason: unknown) {
		if (!this.isCurrent()) return null;
		this.reportError(asError(reason));
		return null;
	}
}

function updateEpoch(
	ref: RefObject<AuthorityEpoch>,
	authority: VirtualPlaybackZonesAuthority | null,
	transport: VirtualPlaybackZonesTransport | null,
) {
	const current = ref.current;
	if (
		current.authorityId === (authority?.authorityId ?? null) &&
		current.showId === (authority?.scope.showId ?? null) &&
		current.transport === transport
	)
		return current;
	ref.current = {
		authorityId: authority?.authorityId ?? null,
		showId: authority?.scope.showId ?? null,
		transport,
		generation: current.generation + 1,
	};
	return ref.current;
}

function initialEpoch(): AuthorityEpoch {
	return {
		authorityId: null,
		showId: null,
		transport: null,
		generation: 0,
	};
}

function sameSnapshot(
	left: VirtualPlaybackZonesSnapshot,
	right: VirtualPlaybackZonesSnapshot,
) {
	return (
		left.revision === right.revision &&
		left.zones.length === right.zones.length &&
		left.zones.every((zone, index) => {
			const other = right.zones[index];
			return (
				other !== undefined &&
				zone.id === other.id &&
				zone.name === other.name &&
				zone.playbackNumbers.length === other.playbackNumbers.length &&
				zone.playbackNumbers.every(
					(number, numberIndex) =>
						number === other.playbackNumbers[numberIndex],
				)
			);
		})
	);
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}

function conflictStatus(reason: unknown) {
	if (!reason || typeof reason !== "object") return null;
	const status = (reason as { status?: unknown }).status;
	return typeof status === "number" ? status : null;
}

const noOp = () => {};
