import { TouchEncoder } from "@tosklight/ui/encoders";
import { HardwareEncoderDisplay } from "../../HardwareEncoderDisplay";
import type { ParameterController } from "../useParameterController";
import { useColorAdoptionNotice } from "../../../../features/familyEncoders/colorAdoptionNotice";
import { familySlotSpreads, familySlotUnsupported } from "./familyEncoderBinding";

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
	const bindingSlot = controller.familyEncoders.componentSlot(index);
	if (!bindingSlot) return null;
	const { descriptor, limits, label } = bindingSlot;
	const display = controller.familyEncoders.display(index);
	// TL-549: angles read back from the displayed output say so; requested values stay unlabelled.
	// TL-551: a Zoom without a published convention is shown quietly and never edited.
	// TL-637: so is a Position slot whose fixtures have no Position physical data.
	const unsupported = familySlotUnsupported(bindingSlot) || display.unsupported === true;
	const startNeeded = notice.required && bindingSlot.component.kind === "color";
	const shownLabel = unsupported
		? `${label} · ${FAMILY_SLOT_UNSUPPORTED_LABEL}`
		: startNeeded
			? `${label} · ${COLOR_START_NEEDED_LABEL}`
			: display.source === "resolved"
			? `${label} · Resolved`
			: label;
	const owner = descriptor.owner;
	const editable =
		bindingSlot.edit === "scalar" && !unsupported && controller.canWriteValues;
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
	if (controller.hardwareConnected)
		return (
			<HardwareEncoderDisplay
				slot={index + 1}
				activateOnHardwarePress
				target={{ label: shownLabel, value: display.text }}
				editValue={display.value === null ? undefined : display.value * scale}
				canRelease={hasScopedValue}
				onEdit={
					editable
						? (next) => controller.familyEncoders.set(index, next / scale)
						: undefined
				}
				onEditRange={setRange}
				onRelease={
					hasScopedValue && controller.canWriteValues
						? () => controller.releaseParameter(owner).then(() => undefined)
						: undefined
				}
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
