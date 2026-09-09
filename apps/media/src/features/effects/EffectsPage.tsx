import { Button, NumberField, SelectField } from "@tosklight/ui/controls";
import { TextField } from "@tosklight/ui/forms";
import {
	DEFAULT_POOL_COLOR_PALETTE,
	PoolCard,
	PoolGrid,
} from "@tosklight/ui/pools";
import { WindowFrame, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useMemo, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { MediaErrorToast } from "../../app/ToastContext";
import { api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import {
	EFFECT_TYPES,
	type EffectLibrarySlot,
	type EffectType,
} from "../../shared/api/effects";
import { useEffects } from "../../shared/api/queries";
import {
	librarySourceGroups,
	type LibrarySourceType,
} from "../media-library/GeneratedLibraryBrowserView";

const EFFECT_SLOT_COUNT = 255;

export function EffectsPage({
	onModeChange,
}: {
	onModeChange?: (mode: LibrarySourceType) => void;
}) {
	const effects = useEffects();
	const editing = useEditing(effects.reload);
	const [selectedSlot, setSelectedSlot] = useState(1);

	return (
		<section className="media-page media-library-page">
			{editing.failure && (
				<MediaErrorToast
					message={editing.failure.message}
					onDismiss={editing.dismiss}
				/>
			)}
			<ResourceState resource={effects} subject="the effects library">
				{(data) => (
					<EffectsLibraryView
						effects={data}
						selectedSlot={selectedSlot}
						busy={editing.busy}
						onSelect={setSelectedSlot}
						onModeChange={onModeChange}
						onSave={(edit) =>
							editing.save(() =>
								api.updateEffect(selectedSlot, {
									requestId: requestId(),
									...edit,
								}),
							)
						}
						onClear={() =>
							editing.save(() =>
								api.updateEffect(selectedSlot, {
									requestId: requestId(),
									clear: true,
								}),
							)
						}
					/>
				)}
			</ResourceState>
		</section>
	);
}

export function EffectsLibraryView({
	effects,
	selectedSlot,
	busy,
	onSelect,
	onModeChange,
	onSave,
	onClear,
}: {
	effects: EffectLibrarySlot[];
	selectedSlot: number;
	busy: boolean;
	onSelect(slot: number): void;
	onModeChange?: (mode: LibrarySourceType) => void;
	onSave(edit: {
		name: string;
		effectType: string;
		parameters: number[];
	}): void;
	onClear(): void;
}) {
	const bySlot = useMemo(
		() => new Map(effects.map((effect) => [effect.slot, effect])),
		[effects],
	);
	const selected = bySlot.get(selectedSlot);
	return (
		<WindowFrame
			title="Library"
			info={{
				primary: "Effects",
				secondary: "Slots 1–255 · selected by the layer effect banks",
			}}
			groups={librarySourceGroups({ value: "effects", onChange: onModeChange })}
			className="media-library-window"
		>
			<div className="media-effects-library-layout">
				<WindowScrollArea className="media-library-pool media-effects-library-pool">
					<div className="media-library-pool-heading">
						<span>Effect slots</span>
						<small>{effects.length}/255 assigned</small>
					</div>
					<PoolGrid
						className="media-file-pool-grid media-effects-pool-grid"
						slotCount={EFFECT_SLOT_COUNT}
						minimumCardWidth={112}
						slots={effects.map((effect) => ({
							id: `effect-${effect.slot}`,
							position: effect.slot - 1,
							card: {
								number: effect.slot,
								primary: effect.name,
								secondary: catalogueLabel(effect),
								color: DEFAULT_POOL_COLOR_PALETTE.dynamic,
								states: effect.slot === selectedSlot ? ["selected"] : [],
							},
						}))}
						emptySlot={(index) => ({
							id: `empty-effect-${index + 1}`,
							position: index,
							card: {
								number: index + 1,
								primary: "Empty",
								secondary: "Available effect slot",
								states: [
									"empty",
									...(index + 1 === selectedSlot
										? (["selected"] as const)
										: []),
								],
							},
						})}
						renderSlot={(slot) => (
							<PoolCard
								model={slot.card}
								onClick={() => onSelect(slot.position + 1)}
							/>
						)}
					/>
				</WindowScrollArea>
				<WindowScrollArea className="media-library-inspector media-effects-library-inspector">
					<EffectSlotEditor
						key={`${selectedSlot}:${selected?.effect.effectType ?? "empty"}`}
						slot={selectedSlot}
						effect={selected}
						busy={busy}
						onSave={onSave}
						onClear={onClear}
					/>
				</WindowScrollArea>
			</div>
		</WindowFrame>
	);
}

function EffectSlotEditor({
	slot,
	effect,
	busy,
	onSave,
	onClear,
}: {
	slot: number;
	effect?: EffectLibrarySlot;
	busy: boolean;
	onSave: Parameters<typeof EffectsLibraryView>[0]["onSave"];
	onClear(): void;
}) {
	const [name, setName] = useState(effect?.name ?? "");
	const [effectType, setEffectType] = useState<EffectType>(
		uiEffectType(effect) ?? EFFECT_TYPES[0].value,
	);
	const [parameters, setParameters] = useState(
		effect?.effect.parameters.map((parameter) => parameter.value) ?? [],
	);

	const save = () => {
		const rasterMode = effectType === "rasterize-cmyk" ? 1 : 0;
		onSave({
			name: name.trim() || effectLabel(effectType),
			effectType: effectType.startsWith("rasterize-")
				? "rasterize"
				: effectType,
			parameters: effectType.startsWith("rasterize-")
				? [rasterMode, ...parameters.slice(1)]
				: parameters,
		});
	};

	return (
		<div className="media-library-editor media-effect-slot-editor">
			<p className="media-library-eyebrow">Effect slot</p>
			<h2>Slot {String(slot).padStart(3, "0")}</h2>
			<p>
				{effect
					? "Edit the effect recalled by this slot."
					: "Assign an effect and its settings to this empty slot."}
			</p>
			<TextField
				label="Name"
				value={name}
				onChange={(event) => setName(event.target.value)}
			/>
			<SelectField
				label="Effect type"
				ariaLabel="Effect type"
				value={effectType}
				options={[...EFFECT_TYPES]}
				onChange={(value) => setEffectType(value as EffectType)}
			/>
			{effect?.effect.parameters.map((parameter, index) => {
				const options = discreteParameterOptions(parameter.id);
				const value = parameters[index] ?? parameter.defaultValue;
				const update = (nextValue: number) =>
					setParameters((current) => {
						const next = [...current];
						next[index] = nextValue;
						return next;
					});
				return options ? (
					<SelectField
						key={parameter.id}
						label={parameter.label}
						ariaLabel={parameter.label}
						value={String(Math.round(value))}
						options={options}
						onChange={(nextValue) => update(Number(nextValue))}
					/>
				) : (
					<NumberField
						key={parameter.id}
						label={parameter.label}
						value={value}
						min={parameter.minimum}
						max={parameter.maximum}
						step={parameter.step}
						allowDecimal={parameter.step < 1}
						onChange={(event) => update(Number(event.target.value))}
					/>
				);
			})}
			{!effect && (
				<p className="media-field-help">
					Assign the effect to load its type-specific settings.
				</p>
			)}
			<div className="media-operator-toolbar">
				<Button variant="primary" disabled={busy} onClick={save}>
					{effect ? "Save effect" : "Assign effect"}
				</Button>
				{effect && (
					<Button disabled={busy} onClick={onClear}>
						Clear slot
					</Button>
				)}
			</div>
		</div>
	);
}

function effectLabel(type: EffectType): string {
	return (
		EFFECT_TYPES.find((candidate) => candidate.value === type)?.label ?? type
	);
}

function uiEffectType(effect?: EffectLibrarySlot): EffectType | undefined {
	if (!effect?.effect.effectType) return undefined;
	if (effect.effect.effectType === "rasterize")
		return (effect.effect.parameters[0]?.value ?? 0) >= 0.5
			? "rasterize-cmyk"
			: "rasterize-bw";
	return EFFECT_TYPES.some(
		(candidate) => candidate.value === effect.effect.effectType,
	)
		? (effect.effect.effectType as EffectType)
		: undefined;
}

function catalogueLabel(effect: EffectLibrarySlot): string {
	const type = uiEffectType(effect);
	return type ? effectLabel(type) : effect.effect.label;
}

function discreteParameterOptions(
	id: string,
): Array<{ value: string; label: string }> | undefined {
	if (id === "blur-type")
		return ["Gaussian", "Shape", "Radial", "Linear", "Axial"].map(
			(label, value) => ({ value: String(value), label }),
		);
	if (id === "feedback-direction")
		return [
			"Top",
			"Bottom",
			"Left",
			"Right",
			"Rotate left",
			"Rotate right",
			"Shake",
			"Tunnel",
		].map((label, value) => ({ value: String(value), label }));
	if (id === "kaleidoscope-repetitions")
		return Array.from({ length: 13 }, (_, value) => ({
			value: String(value),
			label: value === 0 ? "Off" : String(value),
		}));
	return undefined;
}
