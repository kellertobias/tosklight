import {
	Button,
	ColorPickerField,
	FormLayout,
	IconPickerField,
	ModalPortal,
	TextField,
} from "@tosklight/ui";
import {
	INDIVIDUAL_POOL_COLOR_FALLBACK,
	type PoolColorMode,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import type { PoolPresentationConfiguration } from "../../api/types";
import { PoolColorSettings } from "../../components/shared/PoolColorSettings";
import {
	type RecordMode,
	RecordModeDialog,
} from "../../components/shared/RecordModeDialog";
import { resolveConfiguredPoolPresentation } from "../../features/poolPresentation/poolPresentation";
import type { PresetCard } from "../../features/presetRecording/presetCards";
import {
	normalizePresetFamily,
	PRESET_FAMILIES,
	type PresetFamily,
	presetAddress,
	presetStorageKey,
} from "../../presetFamilies";

export type PresetCustomization = {
	title?: string;
	icon?: string;
	color?: string;
};

interface PresetWindowHeaderProps {
	family: PresetFamily;
	onFamily(family: PresetFamily): void;
	onOpenGroups(): void;
	onSettings(anchor: DOMRect): void;
}

export function PresetWindowHeader({
	family,
	onFamily,
	onOpenGroups,
	onSettings,
}: PresetWindowHeaderProps) {
	return (
		<WindowHeader
			title="Preset Pools"
			info={{ primary: `${family} presets` }}
			actions={[
				PRESET_FAMILIES.map((name) => ({
					id: name,
					label: name,
					active: family === name,
					onClick: () => onFamily(name),
				})),
				[{ id: "groups", label: "Groups", onClick: onOpenGroups }],
			]}
			settings
			onSettings={(anchor) => onSettings(anchor.getBoundingClientRect())}
		/>
	);
}

interface PresetCardGridProps {
	cards: readonly (PresetCard | null)[];
	family: PresetFamily;
	customizations: Record<string, PresetCustomization>;
	poolPresentation: PoolPresentationConfiguration;
	showId: string;
	surfaceKey: string;
	fallbackMode: PoolColorMode;
	selectionCount: number;
	storeArmed: boolean;
	updateArmed: boolean;
	setArmed: boolean;
	onActivate(index: number): void;
}

export function PresetCardGrid({
	cards,
	family,
	customizations,
	poolPresentation,
	showId,
	surfaceKey,
	fallbackMode,
	selectionCount,
	storeArmed,
	updateArmed,
	setArmed,
	onActivate,
}: PresetCardGridProps) {
	const slots: PoolSlotViewModel<string>[] = cards.flatMap((preset, index) =>
		preset
			? [
					{
						id: preset.id,
						position: index,
						card: {
							number: `${normalizePresetFamily(preset.body.family)} ${preset.body.number}`,
							primary: preset.body.name,
						},
					},
				]
			: [],
	);
	return (
		<WindowScrollArea>
			<PoolGrid
				slots={slots}
				slotCount={cards.length}
				emptySlot={(index) => ({
					id: presetStorageKey(presetAddress(family, index + 1)),
					position: index,
					card: {
						number: `${family} ${index + 1}`,
						primary: "Empty",
						states: ["empty"],
					},
				})}
				renderSlot={(_, index) => {
					const preset = cards[index] ?? null;
					const storedFamily = normalizePresetFamily(preset?.body.family);
					const filtered = Boolean(preset && storedFamily !== family);
					const id =
						preset?.id ?? presetStorageKey(presetAddress(family, index + 1));
					const customization = customizations[id];
					const presentation = resolveConfiguredPoolPresentation(
						poolPresentation,
						{
							showId,
							surfaceKey,
							fallbackMode,
							objectType: "preset",
							presetFamily:
								storedFamily.toLowerCase() as Lowercase<PresetFamily>,
							itemColorKey: id,
							itemColor: preset?.body.color,
							states: [
								...(!preset ? (["empty"] as const) : []),
								...(filtered ? (["disabled"] as const) : []),
								...(storeArmed ? (["record-target"] as const) : []),
								...(storeArmed ? (["store-target"] as const) : []),
								...(updateArmed ? (["update-target"] as const) : []),
								...(setArmed ? (["set-target"] as const) : []),
							],
						},
					);
					return (
						<Button
							disabled={filtered}
							className={`preset-card pool-cell preset-family-${preset ? storedFamily.toLowerCase() : family.toLowerCase()} ${presentation.className} ${filtered ? "filtered" : ""}`}
							style={presentation.style}
							onClick={() => onActivate(index)}
						>
							<span className="number">{index + 1}</span>
							<PresetCardContent
								preset={preset}
								filtered={filtered}
								storedFamily={storedFamily}
								customization={customization}
								selectionCount={selectionCount}
								storeArmed={storeArmed}
								updateArmed={updateArmed}
							/>
						</Button>
					);
				}}
			/>
		</WindowScrollArea>
	);
}

function PresetCardContent({
	preset,
	filtered,
	storedFamily,
	customization,
	selectionCount,
	storeArmed,
	updateArmed,
}: {
	preset: PresetCard | null;
	filtered: boolean;
	storedFamily: PresetFamily;
	customization?: PresetCustomization;
	selectionCount: number;
	storeArmed: boolean;
	updateArmed: boolean;
}) {
	if (preset && !filtered)
		return (
			<>
				<span className="preset-art">
					{customization?.icon ?? preset.body.icon ?? "◇"}
				</span>
				<b>{customization?.title ?? preset.body.name}</b>
				<small>
					{storedFamily} · {Object.keys(preset.body.values).length} fixtures
				</small>
			</>
		);
	if (filtered) return <small>Other family</small>;
	return (
		<>
			{customization?.icon && (
				<span className="preset-art">{customization.icon}</span>
			)}
			<b>{customization?.title ?? "Empty"}</b>
			<small>
				{updateArmed
					? "Touch to check Update eligibility"
					: selectionCount
						? storeArmed
							? "Record here"
							: "Tap to record programmer"
						: "Select fixtures to record"}
			</small>
		</>
	);
}

interface PresetSettingsProps {
	anchor: DOMRect;
	family: PresetFamily;
	paneId?: string;
	legacyColorsEnabled: boolean;
	onFamily(family: PresetFamily): void;
	onClose(): void;
}

export function PresetSettings({
	anchor,
	family,
	paneId,
	legacyColorsEnabled,
	onFamily,
	onClose,
}: PresetSettingsProps) {
	return (
		<WindowSettings
			modal={false}
			anchor={anchor}
			title="Preset Settings"
			onClose={onClose}
			tabs={[
				{
					id: "pool",
					label: "Pool",
					content: (
						<>
							<h3>Preset family</h3>
							<div className="button-group">
								{PRESET_FAMILIES.map((name) => (
									<Button
										key={name}
										className={family === name ? "active" : ""}
										onClick={() => onFamily(name)}
									>
										{name}
									</Button>
								))}
							</div>
							<PoolColorSettings
								objectType="preset"
								paneId={paneId}
								presetFamily={family.toLowerCase() as Lowercase<PresetFamily>}
								legacyPresetColors={legacyColorsEnabled}
							/>
						</>
					),
				},
			]}
		/>
	);
}

interface PresetCustomizationDialogProps {
	index: number;
	draft: PresetCustomization;
	onDraft(draft: PresetCustomization): void;
	onSave(): void;
	onClose(): void;
}

export function PresetCustomizationDialog({
	index,
	draft,
	onDraft,
	onSave,
	onClose,
}: PresetCustomizationDialogProps) {
	return (
		<ModalPortal onClose={onClose}>
			<div
				className="stacked-modal-layer"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="nested-modal preset-button-settings"
					role="dialog"
					aria-modal="true"
					aria-label="Configure preset button"
				>
					<Button className="modal-close" onClick={onClose}>
						×
					</Button>
					<h3>Configure preset {index + 1}</h3>
					<FormLayout labelPlacement="side">
						<TextField
							label="Title"
							clearable
							value={draft.title ?? ""}
							onChange={(event) =>
								onDraft({ ...draft, title: event.target.value })
							}
						/>
						<IconPickerField
							label="Icon"
							value={draft.icon ?? "◇"}
							onChange={(icon) => onDraft({ ...draft, icon })}
						/>
						<ColorPickerField
							label="Button color"
							value={draft.color ?? INDIVIDUAL_POOL_COLOR_FALLBACK}
							onChange={(color) => onDraft({ ...draft, color })}
						/>
					</FormLayout>
					<footer>
						<Button onClick={onClose}>Cancel</Button>
						<Button className="primary" onClick={onSave}>
							Save button
						</Button>
					</footer>
				</section>
			</div>
		</ModalPortal>
	);
}

interface PresetWindowOverlaysProps {
	settingsAnchor: DOMRect | null;
	family: PresetFamily;
	paneId?: string;
	legacyColorsEnabled: boolean;
	cards: readonly (PresetCard | null)[];
	recordIndex: number | null;
	configureIndex: number | null;
	configureDraft: PresetCustomization;
	onFamily(family: PresetFamily): void;
	onCloseSettings(): void;
	onRecord(index: number, mode: RecordMode): void;
	onCancelRecord(): void;
	onDraft(draft: PresetCustomization): void;
	onCloseConfigure(): void;
	onSaveConfigure(): void;
}

export function PresetWindowOverlays({
	settingsAnchor,
	family,
	paneId,
	legacyColorsEnabled,
	cards,
	recordIndex,
	configureIndex,
	configureDraft,
	onFamily,
	onCloseSettings,
	onRecord,
	onCancelRecord,
	onDraft,
	onCloseConfigure,
	onSaveConfigure,
}: PresetWindowOverlaysProps) {
	const recordTarget = recordIndex == null ? null : cards[recordIndex];
	return (
		<>
			{settingsAnchor && (
				<PresetSettings
					anchor={settingsAnchor}
					family={family}
					paneId={paneId}
					legacyColorsEnabled={legacyColorsEnabled}
					onFamily={onFamily}
					onClose={onCloseSettings}
				/>
			)}
			{recordIndex != null && recordTarget && (
				<RecordModeDialog
					target={recordTarget.body.name ?? `Preset ${recordIndex + 1}`}
					onChoose={(mode) => onRecord(recordIndex, mode)}
					onCancel={onCancelRecord}
				/>
			)}
			{configureIndex != null && (
				<PresetCustomizationDialog
					index={configureIndex}
					draft={configureDraft}
					onDraft={onDraft}
					onClose={onCloseConfigure}
					onSave={onSaveConfigure}
				/>
			)}
		</>
	);
}
