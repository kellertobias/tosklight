import type { KeyboardEvent } from "react";
import { useServer } from "../../../api/ServerContext";
import type {
	ControlActionKind,
	ControlActionSemantic,
	PatchedFixture,
} from "../../../api/types";
import { Button } from "../../common";

export interface CompatibleFixtureControlAction {
	fixtureId: string;
	actionId: string;
	kind: ControlActionKind;
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
	fixtures: PatchedFixture[],
	semantic: ControlActionSemantic,
	selectedFixtureIds: string[] = [],
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

export function ControlDialog() {
	const server = useServer();

	const fixtureControlActions = (
		semantic: ControlActionSemantic,
		allWhenEmpty = false,
	) => {
		if (!server.selectedFixtures.length && !allWhenEmpty) return [];
		return compatibleSpecialDialogActions(
			server.patch?.fixtures ?? [],
			semantic,
			server.selectedFixtures,
		);
	};

	const applyFixtureControl = async (
		semantic: ControlActionSemantic,
		phase: "click" | "press" | "release",
		allWhenEmpty = false,
	) => {
		const actions = fixtureControlActions(semantic, allWhenEmpty).filter(
			(action) =>
				phase === "click"
					? action.kind !== "momentary"
					: action.kind === "momentary",
		);
		if (!actions.length) return;
		await Promise.all(
			actions.map((action) =>
				server.controlFixtureAction(
					action.fixtureId,
					action.actionId,
					phase !== "release",
				),
			),
		);
	};

	const controlButtonProps = (
		semantic: ControlActionSemantic,
		allWhenEmpty = false,
	) => ({
		onClick: () => void applyFixtureControl(semantic, "click", allWhenEmpty),
		onPointerDown: () =>
			void applyFixtureControl(semantic, "press", allWhenEmpty),
		onPointerUp: () =>
			void applyFixtureControl(semantic, "release", allWhenEmpty),
		onPointerCancel: () =>
			void applyFixtureControl(semantic, "release", allWhenEmpty),
		onPointerLeave: () =>
			void applyFixtureControl(semantic, "release", allWhenEmpty),
		onKeyDown: (event: KeyboardEvent) => {
			if (!event.repeat && (event.key === "Enter" || event.key === " ")) {
				void applyFixtureControl(semantic, "press", allWhenEmpty);
			}
		},
		onKeyUp: (event: KeyboardEvent) => {
			if (event.key === "Enter" || event.key === " ") {
				void applyFixtureControl(semantic, "release", allWhenEmpty);
			}
		},
	});

	return (
		<div className="special-action-grid">
			<Button {...controlButtonProps("lamp_on", true)}>Lamps On</Button>
			<Button {...controlButtonProps("lamp_off")}>Lamp Off</Button>
			<Button className="danger" {...controlButtonProps("reset")}>
				Reset
			</Button>
			<Button {...controlButtonProps("fan_auto")}>Fan Auto</Button>
			<Button {...controlButtonProps("fan_low")}>Fan Low</Button>
			<Button {...controlButtonProps("fan_max")}>Fan Max</Button>
		</div>
	);
}
