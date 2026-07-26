import { type KeyboardEvent, useState } from "react";
import { useSelectedPatchedFixtures } from "../../../features/patch/PatchState";
import { useProgrammerActions } from "../../../features/programmerActions/ProgrammerActionsContext";
import type {
	ControlActionKind,
	ControlActionSemantic,
	PatchedFixture,
} from "../../../api/types";
import { Button } from "@tosklight/ui";

export interface CompatibleFixtureControlAction {
	fixtureId: string;
	actionId: string;
	kind: ControlActionKind;
}

export interface AuthoredFixtureControlChoice {
	key: string;
	label: string;
	kind: ControlActionKind;
	durationMillis: number | null;
	actions: CompatibleFixtureControlAction[];
}

function inferredControlSemantic(name: string): ControlActionSemantic {
	const normalized = name
		.trim()
		.toLowerCase()
		.replaceAll(/[^a-z0-9]+/g, " ");
	if (/^(lamp on|strike|ignite)( lamp)?$/.test(normalized)) return "lamp_on";
	if (/^lamp off$/.test(normalized)) return "lamp_off";
	if (/^reset$/.test(normalized)) return "reset";
	if (/^fan auto$/.test(normalized)) return "fan_auto";
	if (/^fan low$/.test(normalized)) return "fan_low";
	if (/^fan high$/.test(normalized)) return "fan_high";
	if (/^fan max(imum)?$/.test(normalized)) return "fan_max";
	return "custom";
}

export function compatibleSpecialDialogActions(
	fixtures: readonly PatchedFixture[],
	semantic: ControlActionSemantic,
	selectedFixtureIds: readonly string[] = [],
): CompatibleFixtureControlAction[] {
	const selected = new Set(selectedFixtureIds);
	return fixtures.flatMap((fixture) => {
		if (
			selected.size &&
			!selected.has(fixture.fixture_id) &&
			!fixture.logical_heads.some((head) => selected.has(head.fixture_id))
		) {
			return [];
		}
		const profile = fixture.definition.profile_snapshot;
		const mode = profile?.modes.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
		);
		if (!mode) return [];
		return mode.control_actions
			.filter(
				(action) =>
					(action.semantic && action.semantic !== "custom"
						? action.semantic
						: inferredControlSemantic(action.name)) === semantic,
			)
			.map((action) => ({
				fixtureId: fixture.fixture_id,
				actionId: action.id,
				kind: action.kind,
			}));
	});
}

export function compatibleAuthoredControlActions(
	fixtures: readonly PatchedFixture[],
	selectedFixtureIds: readonly string[] = [],
): AuthoredFixtureControlChoice[] {
	const selected = new Set(selectedFixtureIds);
	const choices = new Map<string, AuthoredFixtureControlChoice>();
	for (const fixture of fixtures) {
		if (
			selected.size &&
			!selected.has(fixture.fixture_id) &&
			!fixture.logical_heads.some((head) => selected.has(head.fixture_id))
		) {
			continue;
		}
		const profile = fixture.definition.profile_snapshot;
		const mode = profile?.modes.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
		);
		if (!profile || !mode) continue;
		for (const action of mode.control_actions) {
			const key = `${profile.id}:${mode.id}:${action.id}`;
			const choice = choices.get(key) ?? {
				key,
				label: action.name,
				kind: action.kind,
				durationMillis: action.duration_millis,
				actions: [],
			};
			choice.actions.push({
				fixtureId: fixture.fixture_id,
				actionId: action.id,
				kind: action.kind,
			});
			choices.set(key, choice);
		}
	}
	return [...choices.values()];
}

export function ControlDialog({
	selectedFixtureIds,
}: {
	selectedFixtureIds: readonly string[];
}) {
	const programmerActions = useProgrammerActions();
	const selectedFixtures = useSelectedPatchedFixtures(selectedFixtureIds);
	const [activeLatchedActions, setActiveLatchedActions] = useState<Set<string>>(
		() => new Set(),
	);
	const [presetStatus, setPresetStatus] = useState("");
	const authoredActions = compatibleAuthoredControlActions(
		selectedFixtures,
		selectedFixtureIds,
	);
	const selectedPhysicalFixtureIds = selectedFixtureIds.length
		? [...new Set(selectedFixtures.map((fixture) => fixture.fixture_id))]
		: [];

	const fixtureControlActions = (
		semantic: ControlActionSemantic,
		allWhenEmpty = false,
	) => {
		if (!selectedFixtureIds.length && !allWhenEmpty) return [];
		return compatibleSpecialDialogActions(
			selectedFixtures,
			semantic,
			selectedFixtureIds,
		);
	};

	const applyFixtureControl = async (
		actions: readonly CompatibleFixtureControlAction[],
		phase: "click" | "press" | "release",
	) => {
		const applicable = actions.filter((action) =>
			phase === "click"
				? action.kind !== "momentary"
				: action.kind === "momentary",
		);
		if (!applicable.length) return;
		const nextLatchedActions = new Set(activeLatchedActions);
		await Promise.all(
			applicable.map((action) => {
				const key = `${action.fixtureId}:${action.actionId}`;
				const active =
					action.kind === "latched"
						? !activeLatchedActions.has(key)
						: phase !== "release";
				if (action.kind === "latched") {
					if (active) nextLatchedActions.add(key);
					else nextLatchedActions.delete(key);
				}
				return programmerActions?.controlFixtureAction(
					action.fixtureId,
					action.actionId,
					active,
				);
			}),
		);
		if (applicable.some((action) => action.kind === "latched")) {
			setActiveLatchedActions(nextLatchedActions);
		}
	};

	const controlButtonProps = (
		actions: readonly CompatibleFixtureControlAction[],
	) => ({
		onClick: () => void applyFixtureControl(actions, "click"),
		onPointerDown: () => void applyFixtureControl(actions, "press"),
		onPointerUp: () => void applyFixtureControl(actions, "release"),
		onPointerCancel: () => void applyFixtureControl(actions, "release"),
		onPointerLeave: () => void applyFixtureControl(actions, "release"),
		onKeyDown: (event: KeyboardEvent) => {
			if (!event.repeat && (event.key === "Enter" || event.key === " ")) {
				void applyFixtureControl(actions, "press");
			}
		},
		onKeyUp: (event: KeyboardEvent) => {
			if (event.key === "Enter" || event.key === " ") {
				void applyFixtureControl(actions, "release");
			}
		},
	});

	const semanticButtonProps = (
		semantic: ControlActionSemantic,
		allWhenEmpty = false,
	) => controlButtonProps(fixtureControlActions(semantic, allWhenEmpty));

	const generatePortablePresets = async () => {
		if (!selectedPhysicalFixtureIds.length) return;
		setPresetStatus("Generating portable presets…");
		const result = await programmerActions?.generateFixturePresets(
			selectedPhysicalFixtureIds,
		);
		const count = result?.created.length ?? 0;
		setPresetStatus(
			`Created ${count} portable preset${count === 1 ? "" : "s"}`,
		);
	};

	return (
		<div className="control-special-actions">
			<div className="special-action-grid">
				<Button {...semanticButtonProps("lamp_on", true)}>Lamps On</Button>
				<Button {...semanticButtonProps("lamp_off")}>Lamp Off</Button>
				<Button className="danger" {...semanticButtonProps("reset")}>
					Reset
				</Button>
				<Button {...semanticButtonProps("fan_auto")}>Fan Auto</Button>
				<Button {...semanticButtonProps("fan_low")}>Fan Low</Button>
				<Button {...semanticButtonProps("fan_high")}>Fan High</Button>
				<Button {...semanticButtonProps("fan_max")}>Fan Max</Button>
			</div>
			{authoredActions.length > 0 && (
				<section className="authored-control-actions">
					<h3>Fixture controls</h3>
					<div>
						{authoredActions.map((choice) => (
							<Button
								key={choice.key}
								aria-label={`${choice.label} ${choice.kind} control action`}
								{...controlButtonProps(choice.actions)}
							>
								{choice.label}
							</Button>
						))}
					</div>
				</section>
			)}
			<footer>
				<Button
					disabled={!selectedPhysicalFixtureIds.length}
					onClick={() => void generatePortablePresets()}
				>
					Generate portable presets
				</Button>
				{presetStatus && <span role="status">{presetStatus}</span>}
			</footer>
		</div>
	);
}
