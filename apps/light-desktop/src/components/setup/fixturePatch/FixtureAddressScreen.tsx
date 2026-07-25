import { type Dispatch, type SetStateAction, useEffect, useState } from "react";
import type { MultiPatchInstance, PatchedFixture } from "../../../api/types";
import { Button, ModalTitleBar } from "../../common";
import { parsePatchAddress } from "../../input/ConsoleFields";
import { conflicts, firstFreeAddress } from "../patchUtils";
import {
	definitionSplits,
	formatFixturePatch,
	formatInstancePatch,
	splitPatchSetError,
} from "./patchModel";
import { UniverseMap } from "./UniverseMap";

type FixtureAddressScreenProps = {
	fixture: PatchedFixture;
	instance?: MultiPatchInstance;
	fixtures: PatchedFixture[];
	initialSplit: number | null;
	singleValue: string;
	splitValues: Record<number, string>;
	error: string;
	onSingleValue: (value: string) => void;
	onSplitValues: Dispatch<SetStateAction<Record<number, string>>>;
	onCancel: () => void;
	onConfirm: () => void;
};

export function FixtureAddressScreen(props: FixtureAddressScreenProps) {
	const splits = definitionSplits(props.fixture.definition);
	const [activeSplit, setActiveSplit] = useState(
		props.initialSplit ?? splits[0].number,
	);
	const split =
		splits.find((candidate) => candidate.number === activeSplit) ?? splits[0];
	const value =
		splits.length === 1
			? props.singleValue
			: (props.splitValues[split.number] ?? "");
	const parsed = parsePatchAddress(value);
	const setValue = (next: string) =>
		splits.length === 1
			? props.onSingleValue(next)
			: props.onSplitValues((current) => ({
					...current,
					[split.number]: next,
				}));
	const otherFixtures = fixturesWithoutCurrentOwner(
		props.fixtures,
		props.fixture,
		props.instance,
	);
	const validation = validatePendingPatches(
		props.fixture,
		splits,
		props.singleValue,
		props.splitValues,
		otherFixtures,
	);
	useEscapeToCancel(props.onCancel);
	const title = props.instance ? "Multi-patch Address" : "Fixture Address";
	return (
		<section
			className="nested-modal fixture-address-screen"
			role="dialog"
			aria-modal="true"
			aria-label={title}
		>
			<ModalTitleBar
				title={title}
				details={addressDetails(props.fixture, props.instance)}
				actions={
					<Button
						className="primary"
						disabled={validation.invalid}
						onClick={props.onConfirm}
					>
						Set Address
					</Button>
				}
				closeLabel={`Cancel ${title}`}
				onClose={props.onCancel}
			/>
			<AddressSummary
				{...props}
				splits={splits}
				value={value}
				invalid={validation.invalid}
			/>
			{splits.length > 1 && (
				<SplitNavigation
					splits={splits}
					activeSplit={split.number}
					values={props.splitValues}
					onSelect={setActiveSplit}
				/>
			)}
			<div className="fixture-address-content">
				<AddressEntry
					value={value}
					error={props.error}
					invalidMessage={validation.message}
					onValue={setValue}
				/>
				<UniverseMap
					fixtures={otherFixtures}
					universe={parsed?.universe ?? 1}
					proposed={parsed?.address ?? 0}
					footprint={split.footprint}
					proposedLabel={proposalLabel(
						props.fixture,
						props.instance,
						split.number,
					)}
					onAddress={(address) =>
						setValue(`${parsed?.universe ?? 1}.${address}`)
					}
					onUniverse={(universe) =>
						setValue(
							`${universe}.${firstFreeAddress(otherFixtures, universe, split.footprint) ?? 1}`,
						)
					}
				/>
			</div>
		</section>
	);
}

function AddressSummary(
	props: FixtureAddressScreenProps & {
		splits: ReturnType<typeof definitionSplits>;
		value: string;
		invalid: boolean;
	},
) {
	return (
		<div className="fixture-address-summary">
			<span>
				Mode <b>{props.fixture.definition.mode}</b>
			</span>
			<span>
				Complete footprint{" "}
				<b>
					{props.splits.reduce(
						(total, candidate) => total + candidate.footprint,
						0,
					)}{" "}
					slots
				</b>
			</span>
			<span>
				Current{" "}
				<b>
					{props.instance
						? formatInstancePatch(props.fixture.definition, props.instance)
						: formatFixturePatch(props.fixture)}
				</b>
			</span>
			<span>
				Pending{" "}
				<b className={props.invalid ? "invalid" : ""}>
					{props.value || "Unpatched"}
				</b>
			</span>
		</div>
	);
}

function SplitNavigation({
	splits,
	activeSplit,
	values,
	onSelect,
}: {
	splits: ReturnType<typeof definitionSplits>;
	activeSplit: number;
	values: Record<number, string>;
	onSelect: (split: number) => void;
}) {
	return (
		<nav aria-label="Address splits">
			{splits.map((candidate) => (
				<Button
					className={candidate.number === activeSplit ? "active" : ""}
					key={candidate.number}
					onClick={() => onSelect(candidate.number)}
				>
					Split {candidate.number}
					<small>
						{candidate.footprint} slots ·{" "}
						{values[candidate.number] || "Unpatched"}
					</small>
				</Button>
			))}
		</nav>
	);
}

function AddressEntry({
	value,
	invalidMessage,
	error,
	onValue,
}: {
	value: string;
	invalidMessage: string;
	error: string;
	onValue: (value: string) => void;
}) {
	const append = (character: string) =>
		onValue(`${value}${character}`.replace(/^0+(?=\d)/, ""));
	return (
		<div className="fixture-address-entry">
			{/* biome-ignore lint/a11y/noLabelWithoutControl: This styled label is a readout, while the keypad buttons are labeled individually. */}
			<label>
				Universe.address<strong>{value || "—"}</strong>
			</label>
			{/* biome-ignore lint/a11y/useSemanticElements: Keeping the existing div avoids fieldset layout changes in the keypad grid. */}
			<div
				className="fixture-address-number-block"
				role="group"
				aria-label="Fixture address number block"
			>
				{["1", "2", "3", "4", "5", "6", "7", "8", "9", ".", "0", "⌫"].map(
					(key) => (
						<Button
							key={key}
							aria-label={keyLabel(key)}
							onClick={() =>
								key === "⌫" ? onValue(value.slice(0, -1)) : append(key)
							}
						>
							{key}
						</Button>
					),
				)}
			</div>
			<Button className="unpatch" onClick={() => onValue("")}>
				Clear address · Unpatch
			</Button>
			{invalidMessage && <p role="alert">{invalidMessage}</p>}
			{error && <p role="alert">{error}</p>}
		</div>
	);
}

function validatePendingPatches(
	fixture: PatchedFixture,
	splits: ReturnType<typeof definitionSplits>,
	singleValue: string,
	splitValues: Record<number, string>,
	otherFixtures: PatchedFixture[],
) {
	const pending = splits.map((candidate) => {
		const raw =
			splits.length === 1
				? singleValue.trim()
				: (splitValues[candidate.number] ?? "").trim();
		return {
			split: candidate.number,
			raw,
			address: raw ? parsePatchAddress(raw) : null,
			footprint: candidate.footprint,
		};
	});
	const syntaxError = pending.find(
		(candidate) => candidate.raw && !candidate.address,
	);
	const patchError = splitPatchSetError(
		fixture.definition,
		pending.map((candidate) => ({
			split: candidate.split,
			universe: candidate.address?.universe ?? null,
			address: candidate.address?.address ?? null,
		})),
	);
	const occupied = pending.find(
		(candidate) =>
			candidate.address &&
			conflicts(
				otherFixtures,
				candidate.address.universe,
				candidate.address.address,
				candidate.footprint,
			).length,
	);
	return {
		invalid: Boolean(syntaxError || patchError),
		message: syntaxError
			? `Split ${syntaxError.split} must use universe.address.`
			: (patchError ??
				(occupied
					? `The complete Split ${occupied.split} footprint is unavailable at this address.`
					: "")),
	};
}

function fixturesWithoutCurrentOwner(
	fixtures: PatchedFixture[],
	fixture: PatchedFixture,
	instance?: MultiPatchInstance,
) {
	return fixtures.map((candidate) => {
		if (candidate.fixture_id !== fixture.fixture_id) return candidate;
		if (instance)
			return {
				...candidate,
				multipatch: (candidate.multipatch ?? []).filter(
					(item) => item.id !== instance.id,
				),
			};
		return {
			...candidate,
			universe: null,
			address: null,
			split_patches: definitionSplits(candidate.definition).map((split) => ({
				split: split.number,
				universe: null,
				address: null,
			})),
		};
	});
}

function useEscapeToCancel(onCancel: () => void) {
	useEffect(() => {
		const closeOnEscape = (event: KeyboardEvent) => {
			if (event.key === "Escape") onCancel();
		};
		window.addEventListener("keydown", closeOnEscape, true);
		return () => window.removeEventListener("keydown", closeOnEscape, true);
	}, [onCancel]);
}

function addressDetails(
	fixture: PatchedFixture,
	instance?: MultiPatchInstance,
) {
	return instance
		? `Fixture ${fixture.fixture_number ?? fixture.fixture_id} · ${instance.name}`
		: `Fixture ${fixture.fixture_number ?? fixture.fixture_id} · ${fixture.name || fixture.definition.name}`;
}

function proposalLabel(
	fixture: PatchedFixture,
	instance: MultiPatchInstance | undefined,
	split: number,
) {
	return instance
		? `${instance.name} · Split ${split}`
		: `Fixture ${fixture.fixture_number ?? "—"} · Split ${split}`;
}

function keyLabel(key: string) {
	if (key === "⌫") return "Backspace address";
	if (key === ".") return "Universe separator";
	return `Address ${key}`;
}
