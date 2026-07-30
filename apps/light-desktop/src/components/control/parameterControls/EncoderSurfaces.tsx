import { TouchEncoder } from "@tosklight/ui/encoders";
import { useDynamicEditorSession } from "../../../features/dynamics/DynamicEditorSessionContext";
import {
	useProgrammingCommandLineActions,
	useProgrammingDeleteCommandActive,
} from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { HardwareEncoderDisplay } from "../HardwareEncoderDisplay";
import {
	type IndexedPresetChoice,
	indexedPresetChoices,
} from "./indexedPresetChoices";
import { formatNormalizedValue, parameterLabels } from "./model";
import { ProgrammerDynamicsSurface } from "./ProgrammerDynamicsSurface";
import type { ParameterController } from "./useParameterController";

function attributeColor(attribute: string) {
	return (
		{
			"color.red": "#ff3d45",
			"color.green": "#35d568",
			"color.blue": "#378eff",
			"color.white": "#ffffff",
			"color.amber": "#ffb30f",
			"color.uv": "#9a55ff",
		} as Record<string, string>
	)[attribute];
}

function UnassignedEncoder({
	hardwareConnected,
	index,
}: {
	hardwareConnected: boolean;
	index: number;
}) {
	return hardwareConnected ? (
		<HardwareEncoderDisplay slot={index + 1} />
	) : (
		<div
			className="parameter-placeholder"
			role="img"
			aria-label={`Encoder ${index + 1} unassigned`}
		>
			<span>Enc {index + 1}</span>
			<small>Unassigned</small>
		</div>
	);
}

function indexedPresetConfiguration(indexedPresets: IndexedPresetChoice[]) {
	return {
		valueTabLabel: "Direct input",
		presetsTabLabel: "Indexed Presets",
		showWhenEmpty: true,
		emptyMessage:
			"No selected fixture provides a fixed, indexed, or control function for this attribute.",
		groups: [
			{
				label: "Fixed and indexed",
				options: indexedPresets
					.filter((choice) => choice.kind !== "control")
					.map(indexedPresetOption),
			},
			{
				label: "Control actions",
				options: indexedPresets
					.filter((choice) => choice.kind === "control")
					.map(indexedPresetOption),
			},
		],
	};
}

function applyIndexedPresetChoice(
	controller: ParameterController,
	attribute: string,
	choice: IndexedPresetChoice,
) {
	if (choice.disabled) return;
	if (choice.semanticId) {
		void controller.applyIndexedPreset(
			attribute,
			choice.semanticId,
			choice.targets,
		);
		return;
	}
	const actions = controller.programmerActions;
	if (!actions) return;
	const targets = choice.targets.flatMap((target) =>
		target.actionId && target.profileRevision != null
			? [
					{
						fixtureId: target.fixtureId,
						actionId: target.actionId,
						expectedProfileRevision: target.profileRevision,
					},
				]
			: [],
	);
	if (
		actions.controlFixtureActions &&
		targets.length === choice.targets.length
	) {
		const activate = actions.controlFixtureActions(
			targets,
			controller.selectionRevision,
			true,
		);
		void (choice.controlKind === "momentary"
			? activate.then(() =>
					actions.controlFixtureActions?.(
						targets,
						controller.selectionRevision,
						false,
					),
				)
			: activate);
		return;
	}
	void Promise.all(
		choice.targets.flatMap((target) => {
			if (!target.actionId) return [];
			const activate = actions.controlFixtureAction(
				target.fixtureId,
				target.actionId,
				true,
			);
			return choice.controlKind === "momentary"
				? [
						activate.then(() =>
							actions.controlFixtureAction(
								target.fixtureId,
								target.actionId as string,
								false,
							),
						),
					]
				: [activate];
		}),
	);
}

function EncoderSurface({
	controller,
	attribute,
	index,
	deleteArmed,
	resetCommandLine,
}: {
	controller: ParameterController;
	attribute: string | null;
	index: number;
	deleteArmed: boolean;
	resetCommandLine(): Promise<boolean>;
}) {
	if (!attribute)
		return (
			<UnassignedEncoder
				hardwareConnected={controller.hardwareConnected}
				index={index}
			/>
		);
	const value =
		controller.programmerTarget(attribute) ??
		controller.normalized.get(attribute) ??
		0;
	const discrete = controller.encoderDiscreteDisplay(attribute);
	const display =
		controller.encoderNormalizedDisplay(attribute) ??
		formatNormalizedValue(value);
	const hasScopedValue = controller.hasProgrammerValue(attribute);
	const label =
		controller.attributeLabels.get(attribute) ??
		parameterLabels[attribute] ??
		attribute.replaceAll(".", " ");
	const indexedPresets = indexedPresetChoices(
		controller.selectedFixtures,
		controller.selectedFixtureIds,
		attribute,
	);
	const presetConfig = indexedPresetConfiguration(indexedPresets);
	const selectIndexedPreset = (id: string) => {
		const choice = indexedPresets.find((candidate) => candidate.id === id);
		if (choice) applyIndexedPresetChoice(controller, attribute, choice);
	};
	if (controller.hardwareConnected)
		return (
			<HardwareEncoderDisplay
				slot={index + 1}
				activateOnHardwarePress
				target={{ label, value: discrete ?? display }}
				editValue={discrete ? undefined : value * 100}
				canRelease={hasScopedValue}
				presets={presetConfig}
				onPresetSelect={selectIndexedPreset}
				onEdit={
					discrete || !controller.canWriteValues
						? undefined
						: (next) =>
								void controller.applyParameter(
									attribute,
									Math.max(0, Math.min(100, next)) / 100,
								)
				}
				onEditRange={
					discrete || !controller.canWriteValues
						? undefined
						: (points) => void controller.applyParameterRange(attribute, points)
				}
				onRelease={
					hasScopedValue && controller.canWriteValues
						? () => controller.releaseParameter(attribute).then(() => undefined)
						: undefined
				}
				onHardwarePress={() => {
					if (!deleteArmed) return false;
					if (hasScopedValue && controller.canWriteValues) {
						void controller
							.releaseParameter(attribute)
							.then(() => resetCommandLine());
					}
					return true;
				}}
			/>
		);
	return (
		<TouchEncoder
			label={`Enc ${index + 1} · ${label}`}
			slot={index + 1}
			attributeLabel={label}
			value={value}
			display={discrete ?? display}
			accentColor={attributeColor(attribute)}
			mode={controller.dynamicsMode ? "Dynamics" : undefined}
			disabled={!controller.canWriteValues}
			indexed={Boolean(discrete)}
			canRelease={hasScopedValue}
			presets={presetConfig}
			onPresetSelect={selectIndexedPreset}
			onStep={(delta, undoGroup) =>
				void controller.stepParameter(attribute, delta, undoGroup)
			}
			onSet={(next) => void controller.applyParameter(attribute, next)}
			onSetRange={
				discrete || !controller.canWriteValues
					? undefined
					: (points) => void controller.applyParameterRange(attribute, points)
			}
			onRelease={() => void controller.releaseParameter(attribute)}
		/>
	);
}

function indexedPresetOption(choice: IndexedPresetChoice) {
	return {
		value: choice.id,
		label: choice.label,
		description: choice.description,
		disabled: choice.disabled,
	};
}

export function EncoderSurfaces({
	controller,
}: {
	controller: ParameterController;
}) {
	const editor = useDynamicEditorSession();
	const deleteArmed = useProgrammingDeleteCommandActive(
		controller.hardwareConnected,
	);
	const commandLineActions = useProgrammingCommandLineActions();
	if (editor.session || controller.dynamicsMode)
		return <ProgrammerDynamicsSurface controller={controller} />;
	if (!controller.selectedFixtureIds.length && !controller.selectedGroupId)
		return (
			<div className="parameter-empty">
				<b>No fixtures selected</b>
				<small>Select fixtures to inspect or edit their real parameters.</small>
			</div>
		);
	return (
		<>
			{controller.encoderSlots.map((attribute, index) => (
				<EncoderSurface
					key={attribute ?? `empty-${index}`}
					controller={controller}
					attribute={attribute}
					index={index}
					deleteArmed={deleteArmed}
					resetCommandLine={() =>
						commandLineActions?.reset() ?? Promise.resolve(false)
					}
				/>
			))}
		</>
	);
}
