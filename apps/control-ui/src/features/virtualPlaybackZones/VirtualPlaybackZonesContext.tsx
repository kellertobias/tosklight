import {
	createContext,
	type MutableRefObject,
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
import { validateVirtualPlaybackZoneSurfaceId } from "./wire";

interface VirtualPlaybackZonesProviderProps {
	readonly authority: VirtualPlaybackZonesAuthority | null;
	readonly transport: VirtualPlaybackZonesTransport | null;
}

interface AuthorityEpoch {
	readonly authorityId: string | null;
	readonly showId: string | null;
	readonly deskId: string | null;
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
	const [reportedError, setReportedError] = useState<ReportedError | null>(null);
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
									? { generation: epoch.generation, message: error.message }
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
			getSurface: (surfaceId) => controller?.getSurface(surfaceId) ?? null,
			isSavingSurface: (surfaceId) =>
				controller?.isSavingSurface(surfaceId) ?? false,
			subscribeSurface: (surfaceId, listener) =>
				controller?.subscribeSurface(surfaceId, listener) ?? noOp,
			activateSurface: (surfaceId) =>
				controller?.activateSurface(surfaceId) ?? noOp,
			loadSurface: (surfaceId) =>
				controller?.loadSurface(surfaceId) ?? Promise.resolve(null),
			saveSurface: (surfaceId, zones) =>
				controller?.saveSurface(surfaceId, zones) ?? Promise.resolve(null),
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
	private readonly surfaceCache = new Map<
		string,
		readonly VirtualPlaybackZone[]
	>();
	private readonly surfaceVersions = new Map<string, number>();
	private readonly surfaceSaveCounts = new Map<string, number>();
	private readonly surfaceListeners = new Map<string, Set<() => void>>();
	private readonly activeSurfaceCounts = new Map<string, number>();
	private eventStream: VirtualPlaybackZonesEventStream | null = null;
	private snapshotLoaded = false;
	private mutationVersion = 0;

	constructor(
		private readonly scope: VirtualPlaybackZonesScope,
		private readonly transport: VirtualPlaybackZonesTransport,
		private readonly isCurrent: () => boolean,
		private readonly reportError: (error: Error | null) => void,
	) {}

	loadSurface(surfaceId: string) {
		try {
			validateVirtualPlaybackZoneSurfaceId(surfaceId);
		} catch (reason) {
			return Promise.resolve(this.failure(reason));
		}
		const cached = this.getSurface(surfaceId);
		if (cached) return Promise.resolve(cached);
		return this.loadSnapshot().then((snapshot) =>
			snapshot ? this.getSurface(surfaceId) : null,
		);
	}

	getSurface(surfaceId: string) {
		if (!this.isCurrent()) return null;
		const cached = this.surfaceCache.get(surfaceId);
		if (cached) return cached;
		return this.snapshotLoaded ? EMPTY_ZONES : null;
	}

	isSavingSurface(surfaceId: string) {
		return this.isCurrent() && (this.surfaceSaveCounts.get(surfaceId) ?? 0) > 0;
	}

	subscribeSurface(surfaceId: string, listener: () => void) {
		const listeners = this.surfaceListeners.get(surfaceId) ?? new Set();
		listeners.add(listener);
		this.surfaceListeners.set(surfaceId, listeners);
		return () => {
			listeners.delete(listener);
			if (listeners.size === 0) this.surfaceListeners.delete(surfaceId);
		};
	}

	activateSurface(surfaceId: string) {
		try {
			validateVirtualPlaybackZoneSurfaceId(surfaceId);
		} catch (reason) {
			this.failure(reason);
			return noOp;
		}
		const count = this.activeSurfaceCounts.get(surfaceId) ?? 0;
		this.activeSurfaceCounts.set(surfaceId, count + 1);
		if (this.activeSurfaceCount() === 1) this.openWindow();
		let active = true;
		return () => {
			if (!active) return;
			active = false;
			const current = this.activeSurfaceCounts.get(surfaceId) ?? 0;
			if (current <= 1) this.activeSurfaceCounts.delete(surfaceId);
			else this.activeSurfaceCounts.set(surfaceId, current - 1);
			if (this.activeSurfaceCount() === 0) {
				this.eventStream?.close();
				this.eventStream = null;
			}
		};
	}

	private activeSurfaceCount() {
		let count = 0;
		for (const active of this.activeSurfaceCounts.values()) count += active;
		return count;
	}

	private openWindow() {
		this.snapshotLoaded = false;
		this.surfaceCache.clear();
		this.surfaceVersions.clear();
		for (const surfaceId of this.surfaceListeners.keys())
			this.notifySurface(surfaceId);
		this.eventStream = this.transport.subscribe?.(this.scope, {
			changed: (change) => {
				if (
					this.isCurrent() &&
					change.showId === this.scope.showId &&
					change.deskId === this.scope.deskId &&
					this.activeSurfaceCounts.has(change.surfaceId)
				)
					void this.reloadSnapshot();
			},
			gap: () => {
				if (this.isCurrent()) void this.reloadSnapshot();
			},
			error: (error) => {
				if (this.isCurrent()) this.reportError(error);
			},
			closed: () => {
				if (this.isCurrent() && this.activeSurfaceCount() > 0)
					this.reportError(
						new Error("Virtual Playback zone event connection closed"),
					);
			},
		}) ?? null;
	}

	private reloadSnapshot() {
		return this.loadSnapshot();
	}

	saveSurface(
		surfaceId: string,
		zones: readonly VirtualPlaybackZone[],
	) {
		try {
			validateVirtualPlaybackZoneSurfaceId(surfaceId);
			this.changeSaveCount(surfaceId, 1);
			return this.enqueueSave(() => this.performSave(surfaceId, zones)).finally(
				() => this.changeSaveCount(surfaceId, -1),
			);
		} catch (reason) {
			return Promise.resolve(this.failure(reason));
		}
	}

	private changeSaveCount(surfaceId: string, delta: 1 | -1) {
		const count = Math.max(0, (this.surfaceSaveCounts.get(surfaceId) ?? 0) + delta);
		if (count === 0) this.surfaceSaveCounts.delete(surfaceId);
		else this.surfaceSaveCounts.set(surfaceId, count);
		if (this.isCurrent()) this.notifySurface(surfaceId);
	}

	private async performSave(
		surfaceId: string,
		zones: readonly VirtualPlaybackZone[],
	) {
		try {
			const outcome = await this.transport.saveSurface(
				this.scope,
				surfaceId,
				zones,
				crypto.randomUUID(),
			);
			if (!this.isCurrent()) return null;
			if (outcome.surfaceId !== surfaceId)
				throw new Error("Virtual Playback zone response changed surface identity");
			this.mutationVersion += 1;
			this.storeSurface(surfaceId, outcome.zones, this.mutationVersion);
			this.reportError(null);
			return outcome.zones;
		} catch (reason) {
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

	private loadSnapshot() {
		if (this.pendingSnapshot) return this.pendingSnapshot;
		const load = this.performLoad();
		this.pendingSnapshot = load;
		void load.finally(() => {
			if (this.pendingSnapshot === load) this.pendingSnapshot = null;
		});
		return load;
	}

	private async performLoad() {
		const loadVersion = this.mutationVersion;
		try {
			const snapshot = await this.transport.loadSnapshot(this.scope);
			if (!this.isCurrent()) return null;
			if (snapshot.showId !== this.scope.showId)
				throw new Error("Virtual Playback zone response changed authority scope");
			this.installSnapshot(snapshot, loadVersion);
			this.reportError(null);
			return snapshot;
		} catch (reason) {
			return this.failure(reason);
		}
	}

	private installSnapshot(
		snapshot: VirtualPlaybackZonesSnapshot,
		loadVersion: number,
	) {
		const surfaceIds = new Set([
			...this.surfaceCache.keys(),
			...this.surfaceListeners.keys(),
			...Object.keys(snapshot.desks[this.scope.deskId] ?? {}),
		]);
		for (const surfaceId of surfaceIds) {
			if ((this.surfaceVersions.get(surfaceId) ?? 0) > loadVersion) continue;
			this.storeSurface(
				surfaceId,
				snapshot.desks[this.scope.deskId]?.[surfaceId] ?? EMPTY_ZONES,
				loadVersion,
			);
		}
		this.snapshotLoaded = true;
	}

	private storeSurface(
		surfaceId: string,
		zones: readonly VirtualPlaybackZone[],
		version: number,
	) {
		const previous = this.surfaceCache.get(surfaceId);
		this.surfaceVersions.set(surfaceId, version);
		if (previous && sameZones(previous, zones)) return;
		this.surfaceCache.set(surfaceId, zones);
		this.notifySurface(surfaceId);
	}

	private notifySurface(surfaceId: string) {
		for (const listener of this.surfaceListeners.get(surfaceId) ?? []) listener();
	}

	private failure(reason: unknown) {
		if (!this.isCurrent()) return null;
		this.reportError(asError(reason));
		return null;
	}
}

function updateEpoch(
	ref: MutableRefObject<AuthorityEpoch>,
	authority: VirtualPlaybackZonesAuthority | null,
	transport: VirtualPlaybackZonesTransport | null,
) {
	const current = ref.current;
	if (
		current.authorityId === (authority?.authorityId ?? null) &&
		current.showId === (authority?.scope.showId ?? null) &&
		current.deskId === (authority?.scope.deskId ?? null) &&
		current.transport === transport
	)
		return current;
	ref.current = {
		authorityId: authority?.authorityId ?? null,
		showId: authority?.scope.showId ?? null,
		deskId: authority?.scope.deskId ?? null,
		transport,
		generation: current.generation + 1,
	};
	return ref.current;
}

function initialEpoch(): AuthorityEpoch {
	return {
		authorityId: null,
		showId: null,
		deskId: null,
		transport: null,
		generation: 0,
	};
}

function asError(reason: unknown) {
	return reason instanceof Error ? reason : new Error(String(reason));
}

const EMPTY_ZONES: readonly VirtualPlaybackZone[] = [];
const noOp = () => {};

function sameZones(
	left: readonly VirtualPlaybackZone[],
	right: readonly VirtualPlaybackZone[],
) {
	return (
		left.length === right.length &&
		left.every((zone, index) => {
			const other = right[index];
			return (
				other !== undefined &&
				zone.id === other.id &&
				zone.name === other.name &&
				zone.slots.length === other.slots.length &&
				zone.slots.every((slot, slotIndex) => slot === other.slots[slotIndex])
			);
		})
	);
}
