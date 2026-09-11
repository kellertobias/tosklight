import type {
	PatchFixtureProjection,
	PatchProfileRevision,
	PatchSplitAssignment,
} from "@tosklight/patch";

/** The patch as the DMX screen reads it: who occupies one address of one universe. */
export interface DmxOccupant {
	fixture: PatchFixtureProjection;
	/** "Fixture patch", or the multi-patch copy's name. */
	owner: string;
	split: number;
	/** First address of the range this split occupies. */
	start: number;
	footprint: number;
	/** 1-based channel of the split's footprint. */
	channel: number;
	/** The slot's attribute as the fixture library names it, when the profile is embedded. */
	attribute: string | null;
}

/** Every patched address of every universe, in the order the rig lists its fixtures. */
export type DmxOccupancy = ReadonlyMap<number, ReadonlyMap<number, DmxOccupant[]>>;

export const DMX_SLOTS = 512;

/** How many channels fit one row, as the desk's DMX window lays them out. */
export function dmxChannelsPerRow(width: number, cell: number) {
	const usable = Math.max(160, width - 72);
	return Math.max(1, Math.min(DMX_SLOTS, Math.floor((usable + 3) / (cell + 3))));
}

export function dmxOccupancy(
	fixtures: readonly PatchFixtureProjection[],
	profileRevisions: readonly PatchProfileRevision[],
): DmxOccupancy {
	const universes = new Map<number, Map<number, DmxOccupant[]>>();
	for (const fixture of fixtures) {
		const revision = profileRevisions.find(
			(candidate) =>
				candidate.profileId === fixture.profileId &&
				candidate.profileRevision === fixture.profileRevision,
		);
		const footprints = new Map(
			(
				revision?.referencedModes.find((mode) => mode.modeId === fixture.modeId)
					?.splits ?? []
			).map((split) => [split.split, split.footprint]),
		);
		const labels = slotLabels(fixture, revision);
		const owners: Array<{ owner: string; splits: readonly PatchSplitAssignment[] }> =
			[
				{ owner: "Fixture patch", splits: fixture.splitPatches },
				...fixture.multipatch.map((copy, index) => ({
					owner: copy.name.trim() || `Multi-patch ${index + 1}`,
					splits: copy.splitPatches,
				})),
			];
		for (const { owner, splits } of owners) {
			for (const patch of splits) {
				if (patch.universe == null || patch.address == null) continue;
				const footprint = Math.max(1, footprints.get(patch.split) ?? 1);
				const addresses =
					universes.get(patch.universe) ?? new Map<number, DmxOccupant[]>();
				universes.set(patch.universe, addresses);
				for (let channel = 1; channel <= footprint; channel += 1) {
					const address = patch.address + channel - 1;
					if (address > DMX_SLOTS) break;
					const occupants = addresses.get(address) ?? [];
					occupants.push({
						fixture,
						owner,
						split: patch.split,
						start: patch.address,
						footprint,
						channel,
						attribute: labels.get(patch.split)?.get(channel) ?? null,
					});
					addresses.set(address, occupants);
				}
			}
		}
	}
	return universes;
}

/** The universes the show patches, in order. */
export function patchedUniverses(occupancy: DmxOccupancy): number[] {
	return [...occupancy.keys()].sort((left, right) => left - right);
}

/** How the fixture is named on this screen: its Fixture ID and its name. */
export function fixtureTitle(fixture: PatchFixtureProjection) {
	const id =
		fixture.fixtureNumber != null
			? String(fixture.fixtureNumber)
			: fixture.virtualFixtureNumber != null
				? `0.${fixture.virtualFixtureNumber}`
				: null;
	const name = fixture.name.trim() || "Unnamed fixture";
	return id ? `${id} · ${name}` : name;
}

/**
 * Every slot's name, per split, from the profile revision the show embedded.
 *
 * Slots are derived as the encoding plan derives them: within a split, channels take their
 * components in declaration order, each consuming as many slots as its resolution needs.
 */
function slotLabels(
	fixture: PatchFixtureProjection,
	revision: PatchProfileRevision | undefined,
) {
	const labels = new Map<number, Map<number, string>>();
	const mode = revision?.profileSnapshot?.modes.find(
		(candidate) => candidate.id === fixture.modeId,
	);
	if (!mode) return labels;
	const heads = new Map(mode.heads.map((head) => [head.id, head.name]));
	const named = mode.heads.length > 1;
	const next = new Map<number, number>();
	for (const channel of mode.channels ?? []) {
		const split = channel.split ?? 1;
		const first = next.get(split) ?? 1;
		const components = [first, ...(channel.secondary_slots ?? [])];
		next.set(split, first + components.length);
		const splitLabels = labels.get(split) ?? new Map<number, string>();
		labels.set(split, splitLabels);
		components.forEach((offset, index) => {
			const suffix =
				components.length === 1
					? ""
					: index === 0
						? " (coarse)"
						: components.length === 2
							? " (fine)"
							: ` (byte ${index + 1})`;
			const head = named ? heads.get(channel.head_id) : undefined;
			splitLabels.set(
				offset,
				head
					? `${head} · ${channel.attribute}${suffix}`
					: `${channel.attribute}${suffix}`,
			);
		});
	}
	return labels;
}
