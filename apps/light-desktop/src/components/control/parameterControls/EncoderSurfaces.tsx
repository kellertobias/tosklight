import { Button } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import { useState } from "react";
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
	pushTurnAttribute,
	index,
	deleteArmed,
	resetCommandLine,
}: {
	controller: ParameterController;
	attribute: string | null;
	pushTurnAttribute: string | null;
	index: number;
	deleteArmed: boolean;
	resetCommandLine(): Promise<boolean>;
}) {
	const [pushTurnActive, setPushTurnActive] = useState(false);
	if (!attribute)
		return (
			<UnassignedEncoder
				hardwareConnected={controller.hardwareConnected}
				index={index}
			/>
		);
	const activeAttribute =
		pushTurnActive && pushTurnAttribute ? pushTurnAttribute : attribute;
	const value =
		controller.programmerTarget(activeAttribute) ??
		controller.normalized.get(activeAttribute) ??
		0;
	const discrete = controller.encoderDiscreteDisplay(activeAttribute);
	const display =
		controller.encoderNormalizedDisplay(activeAttribute) ??
		formatNormalizedValue(value);
	const hasScopedValue = controller.hasProgrammerValue(activeAttribute);
	const label =
		parameterLabels[activeAttribute] ??
		controller.attributeLabels.get(activeAttribute) ??
		activeAttribute.replaceAll(".", " ");
	const indexedPresets = indexedPresetChoices(
		controller.selectedFixtures,
		controller.selectedFixtureIds,
		activeAttribute,
	);
	const presetConfig = indexedPresetConfiguration(indexedPresets);
	const selectIndexedPreset = (id: string) => {
		const choice = indexedPresets.find((candidate) => candidate.id === id);
		if (choice) applyIndexedPresetChoice(controller, activeAttribute, choice);
	};
	const surface = controller.hardwareConnected ? (
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
								activeAttribute,
								Math.max(0, Math.min(100, next)) / 100,
							)
			}
			onEditRange={
				discrete || !controller.canWriteValues
					? undefined
					: (points) =>
							void controller.applyParameterRange(activeAttribute, points)
			}
			onRelease={
				hasScopedValue && controller.canWriteValues
					? () =>
							controller.releaseParameter(activeAttribute).then(() => undefined)
					: undefined
			}
			onHardwarePress={() => {
				if (!deleteArmed) return false;
				if (hasScopedValue && controller.canWriteValues) {
					void controller
						.releaseParameter(activeAttribute)
						.then(() => resetCommandLine());
				}
				return true;
			}}
		/>
	) : (
		<TouchEncoder
			label={`Enc ${index + 1} · ${label}`}
			slot={index + 1}
			attributeLabel={label}
			value={value}
			display={discrete ?? display}
			accentColor={attributeColor(activeAttribute)}
			mode={controller.dynamicsMode ? "Dynamics" : undefined}
			disabled={!controller.canWriteValues}
			indexed={Boolean(discrete)}
			canRelease={hasScopedValue}
			presets={presetConfig}
			onPresetSelect={selectIndexedPreset}
			onStep={(delta, undoGroup) =>
				void controller.stepParameter(activeAttribute, delta, undoGroup)
			}
			onSet={(next) => void controller.applyParameter(activeAttribute, next)}
			onSetRange={
				discrete || !controller.canWriteValues
					? undefined
					: (points) =>
							void controller.applyParameterRange(activeAttribute, points)
			}
			onRelease={() => void controller.releaseParameter(activeAttribute)}
		/>
	);
	if (!pushTurnAttribute) return surface;
	const primaryLabel =
		parameterLabels[attribute] ??
		controller.attributeLabels.get(attribute) ??
		attribute.replaceAll(".", " ");
	const pushTurnLabel =
		parameterLabels[pushTurnAttribute] ??
		controller.attributeLabels.get(pushTurnAttribute) ??
		pushTurnAttribute.replaceAll(".", " ");
	return (
		<div className="compound-encoder-surface">
			<Button
				size="compact"
				aria-pressed={pushTurnActive}
				onClick={() => setPushTurnActive((current) => !current)}
			>
				{pushTurnActive
					? `Push-turn · ${pushTurnLabel}`
					: `Turn · ${primaryLabel}`}
			</Button>
			{surface}
		</div>
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
					pushTurnAttribute={controller.encoderPushTurnSlots[index] ?? null}
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
