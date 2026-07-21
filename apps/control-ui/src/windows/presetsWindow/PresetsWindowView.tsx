import type { CSSProperties } from "react";
import {
	Button,
	ColorPickerField,
	FormLayout,
	IconPickerField,
	ModalPortal,
	SwitchField,
	TextField,
} from "../../components/common";
import {
	type RecordMode,
	RecordModeDialog,
} from "../../components/shared/RecordModeDialog";
import {
	ButtonGrid,
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "../../components/window-kit";
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
	colorsEnabled: boolean;
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
	colorsEnabled,
	selectionCount,
	storeArmed,
	updateArmed,
	setArmed,
	onActivate,
}: PresetCardGridProps) {
	return (
		<WindowScrollArea>
			<ButtonGrid className="card-pool">
				{cards.map((preset, index) => {
					const storedFamily = normalizePresetFamily(preset?.body.family);
					const filtered = Boolean(preset && storedFamily !== family);
					const id =
						preset?.id ?? presetStorageKey(presetAddress(family, index + 1));
					const customization = customizations[id];
					return (
						<Button
							disabled={filtered}
							key={index + 1}
							className={`preset-card pool-cell preset-family-${preset ? storedFamily.toLowerCase() : family.toLowerCase()} ${!preset ? "empty" : ""} ${filtered ? "filtered" : ""} ${storeArmed ? "store-target" : ""} ${updateArmed ? "update-target" : ""} ${setArmed ? "set-target" : ""}`}
							style={cardStyle(colorsEnabled, customization)}
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
				})}
			</ButtonGrid>
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
				<span
					className="preset-art"
					style={{ background: `${preset.body.color ?? "#2cb7d6"}44` }}
				>
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

function cardStyle(
	colorsEnabled: boolean,
	customization?: PresetCustomization,
) {
	return colorsEnabled && customization?.color
		? ({ "--preset-family": customization.color } as CSSProperties)
		: undefined;
}

interface PresetSettingsProps {
	anchor: DOMRect;
	family: PresetFamily;
	colorsEnabled: boolean;
	onFamily(family: PresetFamily): void;
	onColors(enabled: boolean): void;
	onClose(): void;
}

export function PresetSettings({
	anchor,
	family,
	colorsEnabled,
	onFamily,
	onColors,
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
							<SwitchField
								label="Enable pool colors"
								checked={colorsEnabled}
								onChange={(event) => onColors(event.target.checked)}
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
		<ModalPortal>
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
							value={draft.color ?? "#d98236"}
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
	colorsEnabled: boolean;
	cards: readonly (PresetCard | null)[];
	recordIndex: number | null;
	configureIndex: number | null;
	configureDraft: PresetCustomization;
	onFamily(family: PresetFamily): void;
	onColors(enabled: boolean): void;
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
	colorsEnabled,
	cards,
	recordIndex,
	configureIndex,
	configureDraft,
	onFamily,
	onColors,
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
					colorsEnabled={colorsEnabled}
					onFamily={onFamily}
					onColors={onColors}
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
