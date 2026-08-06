import { Button } from "@tosklight/ui";
import type {
	FixtureProfile,
	PatchFixtureProjection,
	PatchProfileRevision,
} from "@tosklight/patch";
import { useMemo, useState } from "react";
import { documentSession } from "./document/session";
import type { PreviewParameter, PreviewSet } from "./document/session";

/**
 * Driving the rig from the planning window, with no desk and no network.
 *
 * This is not a second desk. There is no programmer, no command line, no playbacks and no cue
 * stack — these are direct values for looking at a rig. Anything that starts to need cues,
 * tracking or LTP arbitration belongs on a desk, and the answer there is to connect to one.
 *
 * Everything set here is session state of this window: it never reaches the show file, and closing
 * the document drops it.
 */

/** The five parameters Simple mode offers, in the order an operator reaches for them. */
const SIMPLE: readonly { parameter: PreviewParameter; label: string }[] = [
	{ parameter: "intensity", label: "Intensity" },
	{ parameter: "pan", label: "Pan" },
	{ parameter: "tilt", label: "Tilt" },
	{ parameter: "gobo", label: "Gobo" },
];

type Mode = "simple" | "full";

export function PreviewControls({
	fixtures,
	profileRevisions,
	selected,
	onError,
}: {
	fixtures: readonly PatchFixtureProjection[];
	/** The profile revisions the show embeds, which are the authority for its channels. */
	profileRevisions: readonly PatchProfileRevision[];
	/** Fixture ids selected in the patch sheet. */
	selected: readonly string[];
	onError: (reason: unknown) => void;
}) {
	const [mode, setMode] = useState<Mode>("simple");
	const [levels, setLevels] = useState<Record<string, number>>({});
	const [colour, setColour] = useState("#ffffff");
	const [slots, setSlots] = useState<Record<number, number>>({});

	const chosen = useMemo(
		() => fixtures.filter((fixture) => selected.includes(fixture.fixtureId)),
		[fixtures, selected],
	);

	/**
	 * Full DMX is a testing tool, not a batch-programming surface: it shows one fixture's complete
	 * mode, so it is only available when exactly one is selected.
	 */
	const single = chosen.length === 1 ? chosen[0] : null;
	const fullAvailable = single !== null;

	const channels = useMemo(
		() => (single ? modeChannels(single, profileRevisions) : []),
		[single, profileRevisions],
	);

	async function send(set: PreviewSet) {
		try {
			await documentSession.setPreview(set);
		} catch (reason) {
			onError(reason);
		}
	}

	function setSemantic(parameter: PreviewParameter, value: number) {
		setLevels((current) => ({ ...current, [parameter]: value }));
		for (const fixture of chosen) {
			void send({
				kind: "semantic",
				fixture_id: fixture.fixtureId,
				parameter,
				value,
				colour: [0, 0, 0],
			});
		}
	}

	/** One operator gesture, however many channels the fixture expresses a colour with. */
	function chooseColour(hex: string) {
		setColour(hex);
		const rgb = hexToRgb(hex);
		for (const fixture of chosen) {
			void send({
				kind: "semantic",
				fixture_id: fixture.fixtureId,
				parameter: "colour",
				value: 0,
				colour: rgb,
			});
		}
	}

	function setSlot(offset: number, value: number) {
		if (!single) return;
		setSlots((current) => ({ ...current, [offset]: value }));
		void send({
			kind: "slot",
			fixture_id: single.fixtureId,
			split: 1,
			offset,
			value,
		});
	}

	async function clear() {
		try {
			await documentSession.clearPreview(chosen.map((fixture) => fixture.fixtureId));
			setLevels({});
			setSlots({});
			setColour("#ffffff");
		} catch (reason) {
			onError(reason);
		}
	}

	if (chosen.length === 0) {
		return (
			<section className="viz-preview" aria-label="Preview controls">
				<p className="viz-preview-hint">
					Select fixtures in the patch sheet to light them without a desk.
				</p>
			</section>
		);
	}

	return (
		<section className="viz-preview" aria-label="Preview controls">
			<header className="viz-preview-head">
				<h2>
					Preview · {chosen.length} {chosen.length === 1 ? "fixture" : "fixtures"}
				</h2>
				<div role="tablist" className="viz-preview-modes">
					<Button
						role="tab"
						aria-selected={mode === "simple"}
						onClick={() => setMode("simple")}
					>
						Simple
					</Button>
					<Button
						role="tab"
						aria-selected={mode === "full"}
						disabled={!fullAvailable}
						title={
							fullAvailable
								? "Every DMX slot of this fixture's mode"
								: "Full DMX needs exactly one fixture selected"
						}
						onClick={() => setMode("full")}
					>
						Full DMX
					</Button>
					<Button onClick={() => void clear()}>Clear</Button>
				</div>
			</header>

			{mode === "simple" || !fullAvailable ? (
				<div className="viz-preview-simple">
					{SIMPLE.map(({ parameter, label }) => (
						<label key={parameter} className="viz-preview-row">
							<span>{label}</span>
							<input
								type="range"
								min={0}
								max={100}
								value={Math.round((levels[parameter] ?? 0) * 100)}
								aria-label={label}
								onChange={(event) =>
									setSemantic(parameter, Number(event.target.value) / 100)
								}
							/>
							<output>{Math.round((levels[parameter] ?? 0) * 100)}%</output>
						</label>
					))}
					<label className="viz-preview-row">
						<span>Colour</span>
						<input
							type="color"
							value={colour}
							aria-label="Colour"
							onChange={(event) => chooseColour(event.target.value)}
						/>
						<output>{colour}</output>
					</label>
					{!fullAvailable && mode === "full" ? (
						<p className="viz-preview-hint">
							Full DMX needs exactly one fixture selected.
						</p>
					) : null}
				</div>
			) : (
				<div className="viz-preview-full">
					<p className="viz-preview-hint">
						Every slot of {single?.name}'s mode, as the fixture library describes it.
					</p>
					<ol className="viz-preview-slots">
						{channels.map((channel) => (
							<li key={channel.offset}>
								<label>
									<span className="viz-preview-slot-number">{channel.offset}</span>
									<span className="viz-preview-slot-name">{channel.label}</span>
									<input
										type="range"
										min={0}
										max={255}
										value={slots[channel.offset] ?? 0}
										aria-label={`Slot ${channel.offset} ${channel.label}`}
										onChange={(event) =>
											setSlot(channel.offset, Number(event.target.value))
										}
									/>
									<output>{slots[channel.offset] ?? 0}</output>
								</label>
							</li>
						))}
					</ol>
				</div>
			)}
		</section>
	);
}

/** One addressable slot of a fixture's patched mode, named as the fixture library names it. */
interface SlotRow {
	offset: number;
	label: string;
}

/**
 * Every slot of the patched mode, including the fine byte of a multi-byte channel and every
 * logical head, because Full DMX is for testing a fixture exhaustively rather than programming it.
 *
 * The authority is the profile revision this show actually embedded, not whatever the library
 * holds now — the same rule the desk follows, so a show opened on another machine reads the same.
 */
function modeChannels(
	fixture: PatchFixtureProjection,
	profileRevisions: readonly PatchProfileRevision[],
): SlotRow[] {
	const profile = profileRevisions.find(
		(revision) =>
			revision.profileId === fixture.profileId &&
			revision.profileRevision === fixture.profileRevision,
	)?.profileSnapshot;
	const mode = profile?.modes.find((candidate) => candidate.id === fixture.modeId);
	if (!mode) return [];
	const heads = new Map(mode.heads.map((head) => [head.id, head.name]));
	const named = mode.heads.length > 1;
	const rows: SlotRow[] = [];
	// Slots are derived exactly as the encoding plan derives them: channels take their components
	// in declaration order, each consuming as many slots as its resolution needs.
	let next = 1;
	for (const channel of mode.channels ?? []) {
		const components = [next, ...(channel.secondary_slots ?? [])];
		next += components.length;
		// Full DMX addresses one split, which is the fixture's primary block.
		if (channel.split !== 1) continue;
		components.forEach((offset, index) => {
			rows.push({
				offset,
				label: label(
					channel.attribute,
					named ? heads.get(channel.head_id) : undefined,
					index,
					components.length,
				),
			});
		});
	}
	return rows.sort((left, right) => left.offset - right.offset);
}

function label(
	attribute: string,
	head: string | undefined,
	component: number,
	components: number,
) {
	const suffix =
		components === 1
			? ""
			: component === 0
				? " (coarse)"
				: components === 2
					? " (fine)"
					: ` (byte ${component + 1})`;
	return head ? `${head} \u00b7 ${attribute}${suffix}` : `${attribute}${suffix}`;
}

function hexToRgb(hex: string): [number, number, number] {
	const value = hex.replace("#", "");
	const component = (start: number) =>
		Number.parseInt(value.slice(start, start + 2), 16) / 255;
	return [component(0), component(2), component(4)];
}
