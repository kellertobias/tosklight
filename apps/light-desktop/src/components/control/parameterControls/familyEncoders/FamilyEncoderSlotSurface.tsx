import { TouchEncoder } from "@tosklight/ui/encoders";
import { HardwareEncoderDisplay } from "../../HardwareEncoderDisplay";
import type { ParameterController } from "../useParameterController";
import { useColorAdoptionNotice } from "../../../../features/familyEncoders/colorAdoptionNotice";
import { familySlotSpreads, familySlotUnsupported } from "./familyEncoderBinding";
import { useOptionalApp } from "../../../../state/AppContext";
import {
	CREATE_POINT_ACTION,
	OPEN_POINTS_PATCH_ACTION,
} from "../../../setup/points/openPointsPatch";

/** TL-651: the Point slot's readout before any Target while the show holds no 3D Point. */
export const NO_POINTS_LABEL = "No Points";

/** TL-554: quiet suffix while a Direct value's first semantic edit needs an explicit start. */
export const COLOR_START_NEEDED_LABEL = "Choose start";

/** Quiet label suffix of a slot the binding will not edit (unknown Zoom convention, TL-551; no
 * Position physical data, TL-637). */
export const FAMILY_SLOT_UNSUPPORTED_LABEL = "Unsupported";

const UNBOUNDED_WINDOW_STEPS = 1_000;

/**
 * One semantic component encoder (TL-549/550/551 UI foundation). Values are in descriptor
 * units: steps are the descriptor's `step` (slow) and ten steps (fast), exactly what a hardware
 * up/down or left/right detent sends, and the typed entry is scaled by `display_scale`. The
 * readout comes from the displayed-source readouts (Pan/Tilt) or the requested value.
 */
export function FamilyEncoderSlotSurface({
	controller,
	index,
}: {
	controller: ParameterController;
	index: number;
}) {
	const notice = useColorAdoptionNotice();
	const app = useOptionalApp();
	const bindingSlot = controller.familyEncoders.componentSlot(index);
	if (!bindingSlot) return null;
	const { descriptor, limits, label } = bindingSlot;
	const shown = controller.familyEncoders.display(index);
	// TL-549: angles read back from the displayed output say so; requested values stay unlabelled.
	// TL-652: angles resolved from a Target name it instead (From XYZ / From Point).
	// TL-551: a Zoom without a published convention is shown quietly and never edited.
	// TL-637: so is a Position slot whose fixtures have no Position physical data.
	// TL-544 G4: the Point slot is a choice encoder on every surface (detent, step or picker).
	const pointSlot = bindingSlot.edit === "target_reference";
	// TL-651: Origin is always a choice; without a 3D Point there is nothing further to step to,
	// so the slot says so instead of an em dash, and its picker offers Create Point.
	const noPoints =
		pointSlot &&
		!controller.familyEncoders.pointChoices.some((choice) => choice.reference.kind === "point");
	const display = noPoints && shown.text === "—" ? { ...shown, text: NO_POINTS_LABEL } : shown;
	const unsupported = familySlotUnsupported(bindingSlot) || display.unsupported === true;
	const startNeeded = notice.required && bindingSlot.component.kind === "color";
	const shownLabel = unsupported
		? `${label} · ${FAMILY_SLOT_UNSUPPORTED_LABEL}`
		: startNeeded
			? `${label} · ${COLOR_START_NEEDED_LABEL}`
			: display.source === "resolved"
			? `${label} · ${display.provenance ?? "Resolved"}`
			: label;
	const owner = descriptor.owner;
	const editable =
		(bindingSlot.edit === "scalar" || pointSlot) && !unsupported && controller.canWriteValues;
	const hasScopedValue = controller.hasProgrammerValue(owner);
	const scale = descriptor.display_scale || 1;
	// `[THRU]` in the value pad (software, hardware modal, keypad and OSC alike) spreads this
	// component over the ordered selection, exactly as a legacy attribute spreads. Typed points
	// are display units; the binding takes descriptor units, kept inside published limits as a
	// typed single value is.
	const setRange =
		editable && familySlotSpreads(bindingSlot)
			? (points: number[]) =>
					controller.familyEncoders.setRange(
						index,
						points.map((point) => {
							const value = point / scale;
							return limits ? Math.min(limits.max, Math.max(limits.min, value)) : value;
						}),
					)
			: undefined;
	const value = display.value ?? limits?.min ?? 0;
	// Without published limits the fader only needs a finite window around the value; it is a
	// presentation window, never a programming limit (the server clamps nothing here).
	const window = descriptor.step * UNBOUNDED_WINDOW_STEPS;
	const minimum = limits?.min ?? value - window;
	const maximum = limits?.max ?? value + window;
	// TL-651: the picker also creates and manages Points (Show Patch › Points).
	const pointPresets = pointSlot
		? {
				presetsTabLabel: "Points",
				note: noPoints
					? "This show has no Points yet. Create Point adds one to aim at."
					: undefined,
				groups: [
					{
						label: "Target reference",
						options: controller.familyEncoders.pointChoices.map((choice) => ({
							value: choice.value,
							label: choice.label,
						})),
					},
				],
				selectedValue: controller.familyEncoders.pointChoices.find(
					(choice) => choice.label === display.text,
				)?.value,
				actions: app
					? [
							{
								id: "create-point",
								label: "Create Point",
								onPress: () => app.dispatch(CREATE_POINT_ACTION),
							},
							{
								id: "manage-points",
								label: "Manage Points",
								onPress: () => app.dispatch(OPEN_POINTS_PATCH_ACTION),
							},
						]
					: undefined,
			}
		: undefined;
	const choosePoint =
		pointSlot && editable
			? (value: string) => controller.familyEncoders.choosePoint(index, value)
			: undefined;
	if (controller.hardwareConnected)
		return (
			<HardwareEncoderDisplay
				slot={index + 1}
				activateOnHardwarePress
				target={{ label: shownLabel, value: display.text }}
				editValue={display.value === null ? undefined : display.value * scale}
				canRelease={hasScopedValue}
				onEdit={
					editable && !pointSlot
						? (next) => controller.familyEncoders.set(index, next / scale)
						: undefined
				}
				onEditRange={setRange}
				onRelease={
					hasScopedValue && controller.canWriteValues
						? () => controller.releaseParameter(owner).then(() => undefined)
						: undefined
				}
				presets={pointPresets}
				onPresetSelect={choosePoint}
				choices={pointSlot}
			/>
		);
	return (
		<TouchEncoder
			label={`Enc ${index + 1} · ${shownLabel}`}
			slot={index + 1}
			attributeLabel={shownLabel}
			value={value}
			display={display.text}
			disabled={!editable}
			canRelease={hasScopedValue}
			onStep={(delta) => controller.familyEncoders.step(index, delta)}
			onDragEnd={() => controller.familyEncoders.finishGestures()}
			touchInteraction={pointSlot ? "choices" : undefined}
			presets={pointPresets}
			onPresetSelect={choosePoint}
			minimum={minimum}
			maximum={maximum}
			inputScale={scale}
			slowStep={descriptor.step}
			fastStep={descriptor.step * 10}
			onSet={(next) => controller.familyEncoders.set(index, next)}
			onSetRange={setRange}
			onRelease={() => void controller.releaseParameter(owner)}
		/>
	);
}
