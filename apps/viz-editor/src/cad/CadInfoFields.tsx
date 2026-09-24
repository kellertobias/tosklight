/**
 * Info's fields for one fixture or object beyond its name and place: where a lamp is patched, how
 * its bracket and barn doors are set, and the options a generated Venue object is built with.
 */
import type {
	FixtureProfileScenery,
	PatchFixtureWrite,
	PatchSplitAssignment,
} from "@tosklight/patch";
import { CommitNumber, CommitText } from "./cadFields";

type SceneryOptions = NonNullable<PatchFixtureWrite["sceneryOptions"]>;

/** A split's DMX address as the patch writes it, `universe.address`; empty while it is unpatched. */
export function formatPatch(split: Pick<PatchSplitAssignment, "universe" | "address">): string {
	return split.universe != null && split.address != null ? `${split.universe}.${split.address}` : "";
}

/** A typed DMX address: empty unpatches the split; anything but `universe.address` is refused. */
export function parsePatch(text: string): Pick<PatchSplitAssignment, "universe" | "address"> | null {
	const trimmed = text.trim();
	if (!trimmed) return { universe: null, address: null };
	const match = /^(\d+)\.(\d+)$/u.exec(trimmed);
	if (!match) return null;
	const [universe, address] = [Number(match[1]), Number(match[2])];
	return universe >= 1 && address >= 1 && address <= 512 ? { universe, address } : null;
}

/** One address per split of the placement; a fixture with no splits recorded yet shows its first. */
export function PatchFields({
	splits,
	onCommit,
}: {
	splits: readonly PatchSplitAssignment[];
	onCommit(next: PatchSplitAssignment[]): void;
}) {
	const shown = splits.length ? splits : [{ split: 1, universe: null, address: null }];
	return (
		<>
			{shown.map((split) => {
				const label = shown.length > 1 ? `Patch split ${split.split}` : "Patch";
				return (
					<CommitText
						key={split.split}
						label={label}
						placeholder="Unpatched"
						value={formatPatch(split)}
						accepts={(draft) => parsePatch(draft) != null}
						onCommit={(draft) => {
							const address = parsePatch(draft);
							if (!address) return;
							onCommit(
								shown.map((each) => (each.split === split.split ? { ...each, ...address } : each)),
							);
						}}
					/>
				);
			})}
		</>
	);
}

/** A number of degrees, or empty for none. */
export function acceptsOptionalNumber(draft: string) {
	return draft.trim() === "" || Number.isFinite(Number(draft.trim().replace(",", ".")));
}

export function MountingFields({
	bracketAngle,
	shaperAngle,
	onCommit,
}: {
	bracketAngle: number;
	shaperAngle: number | null;
	onCommit(change: { bracketAngle?: number; shaperAngle?: number | null }): void;
}) {
	return (
		<>
			<CommitNumber
				label="Bracket angle"
				unit="°"
				digits={1}
				value={bracketAngle}
				onCommit={(degrees) => onCommit({ bracketAngle: degrees })}
			/>
			<CommitText
				label="Barndoors (°)"
				ariaLabel="Barndoors"
				placeholder="None"
				value={shaperAngle == null ? "" : String(shaperAngle)}
				accepts={acceptsOptionalNumber}
				onCommit={(draft) =>
					onCommit({
						shaperAngle: draft.trim() === "" ? null : Number(draft.trim().replace(",", ".")),
					})
				}
			/>
		</>
	);
}

const CHAIN_ENDS = [
	{ value: "motor", label: "Motor" },
	{ value: "direct", label: "Direct" },
	{ value: "steelflex_loop", label: "Steelflex loop" },
] as const;

function ChainEnd({
	label,
	value,
	onChange,
}: {
	label: string;
	value: string | null | undefined;
	onChange(value: string | null): void;
}) {
	return (
		<label className="cad-field">
			<span>{label}</span>
			<select
				className="ui-input"
				aria-label={label}
				value={value ?? ""}
				onChange={(event) => onChange(event.currentTarget.value || null)}
			>
				<option value="">Default</option>
				{CHAIN_ENDS.map((end) => (
					<option key={end.value} value={end.value}>
						{end.label}
					</option>
				))}
			</select>
		</label>
	);
}

/** The sides a flight of stairs can carry a handrail on, as seen climbing it. */
const HANDRAIL_SIDES = [
	{ value: "none", label: "None" },
	{ value: "left", label: "Left" },
	{ value: "right", label: "Right" },
	{ value: "both", label: "Both sides" },
] as const;

/** Which sides a flight's handrails run up: the choice made for it, else what its profile has. */
function StairHandrails({
	value,
	onChange,
}: {
	value: NonNullable<SceneryOptions["handrails"]>;
	onChange(value: NonNullable<SceneryOptions["handrails"]>): void;
}) {
	return (
		<label className="cad-field">
			<span>Handrails</span>
			<select
				className="ui-input"
				aria-label="Handrails"
				value={value}
				onChange={(event) =>
					onChange(event.currentTarget.value as NonNullable<SceneryOptions["handrails"]>)
				}
			>
				{HANDRAIL_SIDES.map((side) => (
					<option key={side.value} value={side.value}>
						{side.label}
					</option>
				))}
			</select>
		</label>
	);
}

/** The options a generated object is built with: its colour, a chain's two ends, a stair's rails. */
export function SceneryParameters({
	scenery,
	options,
	shared,
	onCommit,
}: {
	scenery: FixtureProfileScenery;
	options: SceneryOptions | null | undefined;
	shared: string;
	onCommit(next: SceneryOptions): void;
}) {
	const current = options ?? {};
	return (
		<div className="cad-info-parameters" role="group" aria-label="Parameters">
			<span>Parameters{shared}</span>
			<CommitText
				label="Colour"
				placeholder="Default"
				value={current.colourSrgb ?? ""}
				accepts={(draft) => draft.trim() === "" || /^#?[0-9a-f]{6}$/iu.test(draft.trim())}
				onCommit={(draft) => {
					const hex = draft.trim().replace(/^#?/u, "#").toLowerCase();
					onCommit({ ...current, colourSrgb: draft.trim() ? hex : null });
				}}
			/>
			{scenery.kind === "chain" ? (
				<>
					<ChainEnd
						label="Chain top"
						value={current.chainTop}
						onChange={(chainTop) =>
							onCommit({ ...current, chainTop: chainTop as SceneryOptions["chainTop"] })
						}
					/>
					<ChainEnd
						label="Chain bottom"
						value={current.chainBottom}
						onChange={(chainBottom) =>
							onCommit({
								...current,
								chainBottom: chainBottom as SceneryOptions["chainBottom"],
							})
						}
					/>
				</>
			) : null}
			{scenery.kind === "stairs" ? (
				<StairHandrails
					value={current.handrails ?? (scenery.handrails ? "both" : "none")}
					onChange={(handrails) => onCommit({ ...current, handrails })}
				/>
			) : null}
		</div>
	);
}
