import {
	Button,
	ColorPickerField,
	FormLayout,
	IconPickerField,
	ModalPortal,
	ModalTitleBar,
	TextField,
} from "@tosklight/ui";
import {
	INDIVIDUAL_POOL_COLOR_FALLBACK,
	PoolCard,
	type PoolColorMode,
	PoolGrid,
	type PoolSlotViewModel,
	type PoolCardSizing,
} from "@tosklight/ui/pools";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import { useMemo } from "react";
import type { PoolPresentationConfiguration } from "../../api/types";
import { PoolColorSettings } from "../../components/shared/PoolColorSettings";
import {
	type RecordMode,
	RecordModeDialog,
} from "../../components/shared/RecordModeDialog";
import { resolveConfiguredPoolPresentation } from "../../features/poolPresentation/poolPresentation";
import { PresetPreviewGlyph } from "../../features/presetPreview/PresetPreviewGlyph";
import {
	type PresetPreview,
	presetIntentPreview,
	presetTileArtwork,
} from "../../features/presetPreview/presetPreview";
import type { PresetCard } from "../../features/presetRecording/presetCards";
import {
	type PresetFixtureCounts,
	presetFixtureCountLabel,
} from "../../features/presetRecording/presetFixtureCounts";
import {
	type PoolMutationTarget,
	poolMutationTargetState,
} from "../../features/controlSurfaceInteraction/poolCommandTarget";
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
	compact?: boolean;
	showFamilyActions?: boolean;
	onFamily(family: PresetFamily): void;
	onOpenGroups(): void;
	onSettings(anchor: DOMRect): void;
}

export function PresetWindowHeader({
	family,
	compact = false,
	showFamilyActions = true,
	onFamily,
	onOpenGroups,
	onSettings,
}: PresetWindowHeaderProps) {
	return (
		<WindowHeader
			title={compact ? `${family} Presets` : "Preset Pools"}
			info={compact ? undefined : { primary: `${family} presets` }}
			groups={[
				{
					id: "preset-family",
					kind: "tabs",
					activeId: family,
					onActiveChange: (id) => onFamily(id as PresetFamily),
					actions: showFamilyActions
					? PRESET_FAMILIES.map((name) => ({
							id: name,
							label: name,
						}))
					: [],
				},
				{ id: "preset-related", actions: [{ id: "groups", label: "Groups", onPress: onOpenGroups }] },
			]}
			settings
			onSettings={(anchor) => onSettings(anchor.getBoundingClientRect())}
		/>
	);
}

interface PresetCardGridProps {
	cards: readonly (PresetCard | null)[];
	family: PresetFamily;
	cardSizing: PoolCardSizing;
	customizations: Record<string, PresetCustomization>;
	poolPresentation: PoolPresentationConfiguration;
	showId: string;
	surfaceKey: string;
	fallbackMode: PoolColorMode;
	selectionCount: number;
	recallReady: boolean;
	storeArmed: boolean;
	updateArmed: boolean;
	setArmed: boolean;
	mutationTarget?: PoolMutationTarget | null;
	/** Active / defined fixture counts per stored Preset id. */
	fixtureCounts?: ReadonlyMap<string, PresetFixtureCounts>;
	/** Ordered Group membership, so a Group value previews one sample per member. */
	groupMembers?: ReadonlyMap<string, readonly string[]>;
	onActivate(index: number): void;
	onConfigure?(index: number): void;
}

type PresetSlotProps = Omit<
	PresetCardGridProps,
	"cards" | "cardSizing" | "groupMembers"
> & {
	index: number;
	preset: PresetCard | null;
	preview: PresetPreview | null;
};

function presetMutationState(
	preset: PresetCard | null,
	filtered: boolean,
	family: PresetFamily,
	mutationTarget: PoolMutationTarget | null,
) {
	const eligible =
		mutationTarget?.phase === "source"
			? preset !== null && !filtered
			: mutationTarget?.phase === "destination"
				? preset === null &&
					mutationTarget.source.startsWith(`${family.toUpperCase()} PRESET `)
				: false;
	return {
		eligible,
		state: eligible ? poolMutationTargetState(mutationTarget) : null,
	};
}

function presetSecondary(
	preset: PresetCard | null,
	filtered: boolean,
	storedFamily: PresetFamily,
	{
		fixtureCounts,
		updateArmed,
		selectionCount,
		storeArmed,
	}: Pick<
		PresetSlotProps,
		"fixtureCounts" | "updateArmed" | "selectionCount" | "storeArmed"
	>,
) {
	if (preset)
		return filtered
			? storedFamily
			: presetFixtureCountLabel(
					fixtureCounts?.get(preset.id) ?? {
						active: 0,
						defined: Object.keys(preset.body.values).length,
						universal:
							Object.keys(preset.body.universal_values ?? {}).length > 0,
					},
				);
	if (updateArmed) return "Touch to check Update eligibility";
	if (!selectionCount) return "Select fixtures to record";
	return storeArmed ? "Record here" : "Tap to record programmer";
}

function PresetPoolSlot(props: PresetSlotProps) {
	const { index, preset, family, storeArmed, updateArmed, setArmed } = props;
	const storedFamily = normalizePresetFamily(preset?.body.family);
	const filtered = Boolean(preset && storedFamily !== family);
	const id = preset?.id ?? presetStorageKey(presetAddress(family, index + 1));
	const customization = props.customizations[id];
	const mutation = presetMutationState(
		preset,
		filtered,
		family,
		props.mutationTarget ?? null,
	);
	const presentation = resolveConfiguredPoolPresentation(
		props.poolPresentation,
		{
			showId: props.showId,
			surfaceKey: props.surfaceKey,
			fallbackMode: props.fallbackMode,
			objectType: "preset",
			presetFamily: storedFamily.toLowerCase() as Lowercase<PresetFamily>,
			itemColorKey: id,
			itemColor: preset?.body.color,
			states: [
				...(!preset ? (["empty"] as const) : []),
				...(filtered ? (["disabled"] as const) : []),
				...(storeArmed ? (["record-target", "store-target"] as const) : []),
				...(updateArmed ? (["update-target"] as const) : []),
				...(setArmed ? (["set-target"] as const) : []),
				...(mutation.state ? [mutation.state] : []),
			],
		},
	);
	// The pool grid stamps each slot's identity onto the element it renders.
	const identity = Object.fromEntries(
		Object.entries(props).filter(([name]) => name.startsWith("data-")),
	);
	const artwork = preset
		? presetTileArtwork(preset.body, customization, filtered ? null : props.preview)
		: { preview: null };
	return (
		<PoolCard
			{...identity}
			disabled={
				filtered ||
				Boolean(
					preset &&
						!props.recallReady &&
						!storeArmed &&
						!updateArmed &&
						!setArmed &&
						!mutation.eligible,
				)
			}
			className={`preset-card preset-family-${preset ? storedFamily.toLowerCase() : family.toLowerCase()} ${presentation.className} ${filtered ? "filtered" : ""}`}
			style={presentation.style}
			onClick={() => props.onActivate(index)}
			onContextMenu={(event) => {
				event.preventDefault();
				props.onConfigure?.(index);
			}}
			model={{
				number: preset?.body.number ?? index + 1,
				primary: filtered
					? "Other family"
					: (customization?.title ?? preset?.body.name ?? "Empty"),
				secondary: presetSecondary(preset, filtered, storedFamily, props),
				icon: artwork.icon,
				preview: artwork.preview && (
					<PresetPreviewGlyph preview={artwork.preview} />
				),
				iconColor: artwork.color,
				color: artwork.color,
				kind: "preset",
				states: presentation.states,
			}}
		/>
	);
}

export function PresetCardGrid({
	cards,
	cardSizing,
	groupMembers,
	...slotProps
}: PresetCardGridProps) {
	const { family } = slotProps;
	const previews = useMemo(
		() =>
			new Map(
				cards.flatMap((preset) =>
					preset ? [[preset.id, presetIntentPreview(preset.body, groupMembers)] as const] : [],
				),
			),
		[cards, groupMembers],
	);
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
				cardSizing={cardSizing}
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
					return (
						<PresetPoolSlot
							{...slotProps}
							index={index}
							preset={preset}
							preview={preset ? (previews.get(preset.id) ?? null) : null}
						/>
					);
				}}
			/>
		</WindowScrollArea>
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
					<ModalTitleBar
						title={`Configure preset ${index + 1}`}
						onClose={onClose}
					/>
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
							value={draft.icon ?? ""}
							onChange={(icon) => onDraft({ ...draft, icon })}
						/>
						<ColorPickerField
							label="Button color"
							value={draft.color ?? INDIVIDUAL_POOL_COLOR_FALLBACK}
							onChange={(color) => onDraft({ ...draft, color })}
						/>
					</FormLayout>
					<footer>
						<Button
							disabled={!draft.icon && !draft.color}
							onClick={() => onDraft({ ...draft, icon: "", color: undefined })}
						>
							Automatic icon
						</Button>
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
