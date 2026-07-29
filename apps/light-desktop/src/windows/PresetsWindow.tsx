import { INDIVIDUAL_POOL_COLOR_FALLBACK } from "@tosklight/ui/pools";
import { useMemo, useState } from "react";
import { useCommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import { requestUpdateTarget } from "../components/control/updateWorkflow";
import { GroupStrip } from "../components/shared/GroupStrip";
import type { RecordMode } from "../components/shared/RecordModeDialog";
import { presets } from "../data/mockData";
import {
	useActiveShowId,
	useBootstrapReady,
} from "../features/deskSnapshot/DeskSnapshotState";
import {
	poolSurfaceKey,
	usePoolPresentationSettings,
} from "../features/poolPresentation/poolPresentation";
import type { PresetRecallActions } from "../features/presetRecall/contracts";
import { usePresetRecall } from "../features/presetRecall/PresetRecallProvider";
import { usePresetRecording } from "../features/presetRecording/PresetRecordingProvider";
import {
	type PresetCard,
	resolvePresetCards,
} from "../features/presetRecording/presetCards";
import { submitPresetRecording } from "../features/presetRecording/submitRecording";
import { useProgrammerActions } from "../features/programmerActions/ProgrammerActionsContext";
import { useProgrammerPreloadLifecycleView } from "../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView";
import { usePresets } from "../features/showObjects/ShowObjectsState";
import {
	normalizePresetFamily,
	presetAddress,
	presetStorageKey,
} from "../presetFamilies";
import { useApp } from "../state/AppContext";
import {
	PresetCardGrid,
	type PresetCustomization,
	PresetWindowHeader,
	PresetWindowOverlays,
} from "./presetsWindow/PresetsWindowView";
import type { WindowProps } from "./windowTypes";

function fallbackPresets(enabled: boolean): PresetCard[] {
	if (!enabled) return [];
	return presets
		.filter((preset) => preset.name)
		.map((preset) => ({
			id: String(preset.id),
			body: {
				name: preset.name ?? "",
				number: preset.id,
				values: {},
				family: normalizePresetFamily(preset.family),
				color: preset.color,
				icon: preset.icon,
			},
		}));
}

interface PresetActivationOptions {
	index: number;
	cards: readonly (PresetCard | null)[];
	family: ReturnType<typeof presetAddress>["family"];
	customizations: Record<string, PresetCustomization>;
	updateArmed: boolean;
	setArmed: boolean;
	storeArmed: boolean;
	actions: PresetRecallActions | null;
	onConfigure(index: number, draft: PresetCustomization): void;
	onStore(index: number, occupied: boolean): void;
	onDisarmSet(): void;
}

function activatePreset(options: PresetActivationOptions) {
	const { index, cards, family, customizations } = options;
	const preset = cards[index];
	const id = preset?.id ?? presetStorageKey(presetAddress(family, index + 1));
	if (options.updateArmed) {
		requestUpdateTarget({ family: { type: "preset" }, object_id: id });
		return;
	}
	if (options.setArmed) {
		const saved = customizations[id] ?? {};
		options.onConfigure(index, {
			title: saved.title ?? preset?.body.name ?? `Preset ${index + 1}`,
			icon: saved.icon ?? preset?.body.icon ?? "◇",
			color:
				saved.color ?? preset?.body.color ?? INDIVIDUAL_POOL_COLOR_FALLBACK,
		});
		options.onDisarmSet();
		return;
	}
	if (!preset && !options.storeArmed) return;
	if (options.storeArmed) options.onStore(index, preset !== null);
	else if (preset)
		void options.actions?.recall({
			objectId: preset.id,
			address: presetAddress(
				normalizePresetFamily(preset.body.family),
				preset.body.number,
			),
		});
}

function usePresetsWindowModel({
	active = true,
	compact,
	paneId,
	showGroupShortcuts,
	presetFamily,
	presetPoolColors,
}: WindowProps) {
	const bootstrapReady = useBootstrapReady();
	const activeShowId = useActiveShowId();
	const programmerActions = useProgrammerActions();
	const presetRecall = usePresetRecall(active);
	const selection = presetRecall.selection;
	const storedPresets = usePresets(active);
	const presetRecording = usePresetRecording();
	const preload = useProgrammerPreloadLifecycleView(active);
	const command = useCommandLineSurface({
		enabled: active,
		observeCommand: false,
	});
	const { state, dispatch } = useApp();
	const poolSettings = usePoolPresentationSettings();
	const family = compact
		? (presetFamily ?? state.presetFamily)
		: state.presetFamily;
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const legacyColorsEnabled = compact
		? (presetPoolColors ?? true)
		: state.presetPoolColors;
	const showId = activeShowId ?? "unresolved";
	const colorSurfaceKey = poolSurfaceKey(showId, "preset", paneId);
	const colorMode =
		poolSettings.configuration.modes[colorSurfaceKey] ??
		(legacyColorsEnabled ? "type" : "individual");
	const customizations = useMemo(() => {
		const prefix = `show:${showId}:preset:`;
		return Object.fromEntries(
			Object.entries(poolSettings.configuration.items)
				.filter(([key]) => key.startsWith(prefix))
				.map(([key, value]) => [key.slice(prefix.length), value]),
		) as Record<string, PresetCustomization>;
	}, [poolSettings.configuration.items, showId]);
	const [configureIndex, setConfigureIndex] = useState<number | null>(null);
	const [configureDraft, setConfigureDraft] = useState<PresetCustomization>({});
	const [recordPresetIndex, setRecordPresetIndex] = useState<number | null>(
		null,
	);
	const setFamily = (next: typeof state.presetFamily) =>
		dispatch(
			compact && paneId
				? { type: "SET_PANE_PRESET_FAMILY", id: paneId, family: next }
				: { type: "SET_PRESET_FAMILY", family: next },
		);
	const groupsVisible = compact
		? Boolean(showGroupShortcuts)
		: state.presetGroupsVisible;
	const fallback = fallbackPresets(!bootstrapReady);
	const stored = activeShowId !== null ? storedPresets : fallback;
	const cards = resolvePresetCards(stored, family);
	const cancelRecording = () => {
		setRecordPresetIndex(null);
		dispatch({ type: "SET_STORE_ARMED", value: false });
	};
	const recordPreset = async (index: number, mode: RecordMode) => {
		setRecordPresetIndex(null);
		dispatch({ type: "SET_STORE_ARMED", value: false });
		const outcome = await submitPresetRecording({
			card: cards[index],
			index,
			family,
			mode,
			preloadActive: preload.armed || preload.active,
			actions: presetRecording,
			storePreload: programmerActions?.storePreload ?? (async () => false),
		});
		if (outcome) await command.reset();
	};

	const activate = (index: number) =>
		activatePreset({
			index,
			cards,
			family,
			customizations,
			updateArmed: state.updateArmed,
			setArmed: state.presetSetArmed,
			storeArmed: state.storeArmed,
			actions: presetRecall.actions,
			onConfigure: (target, draft) => {
				setConfigureIndex(target);
				setConfigureDraft(draft);
			},
			onStore: (target, occupied) =>
				occupied
					? setRecordPresetIndex(target)
					: void recordPreset(target, "overwrite"),
			onDisarmSet: () =>
				dispatch({ type: "SET_PRESET_SET_ARMED", value: false }),
		});
	const saveCustomization = () => {
		if (configureIndex == null) return;
		const id =
			cards[configureIndex]?.id ??
			presetStorageKey(presetAddress(family, configureIndex + 1));
		void poolSettings
			.setItem("preset", id, configureDraft)
			.then(() => setConfigureIndex(null));
	};
	return {
		active,
		compact,
		family,
		colorMode,
		colorSurfaceKey,
		poolPresentation: poolSettings.configuration,
		showId,
		paneId,
		legacyColorsEnabled,
		cards,
		customizations,
		groupsVisible,
		selectionCount: selection?.selected.length ?? 0,
		recallReady: presetRecall.actions !== null,
		storeArmed: state.storeArmed,
		updateArmed: state.updateArmed,
		setArmed: state.presetSetArmed,
		settingsAnchor,
		recordPresetIndex,
		configureIndex,
		configureDraft,
		activate,
		setFamily,
		setSettingsAnchor,
		cancelRecording,
		recordPreset,
		setConfigureDraft,
		setConfigureIndex,
		saveCustomization,
		openGroups: () => dispatch({ type: "OPEN_BUILTIN", kind: "groups" }),
	};
}

export function PresetsWindow(props: WindowProps) {
	const model = usePresetsWindowModel(props);
	return (
		<div
			className={`pool-window preset-pool-window pool-color-mode-${model.colorMode} pool-family-${model.family.toLowerCase()}`}
		>
			{!model.compact && (
				<PresetWindowHeader
					family={model.family}
					onFamily={model.setFamily}
					onOpenGroups={model.openGroups}
					onSettings={model.setSettingsAnchor}
				/>
			)}
			<PresetCardGrid
				cards={model.cards}
				family={model.family}
				customizations={model.customizations}
				poolPresentation={model.poolPresentation}
				showId={model.showId}
				surfaceKey={model.colorSurfaceKey}
				fallbackMode={model.legacyColorsEnabled ? "type" : "individual"}
				selectionCount={model.selectionCount}
				recallReady={model.recallReady}
				storeArmed={model.storeArmed}
				updateArmed={model.updateArmed}
				setArmed={model.setArmed}
				onActivate={model.activate}
			/>
			{model.groupsVisible && <GroupStrip active={model.active} />}
			<PresetWindowOverlays
				settingsAnchor={model.settingsAnchor}
				family={model.family}
				paneId={model.paneId}
				legacyColorsEnabled={model.legacyColorsEnabled}
				cards={model.cards}
				recordIndex={model.recordPresetIndex}
				configureIndex={model.configureIndex}
				configureDraft={model.configureDraft}
				onFamily={model.setFamily}
				onCloseSettings={() => model.setSettingsAnchor(null)}
				onRecord={model.recordPreset}
				onCancelRecord={model.cancelRecording}
				onDraft={model.setConfigureDraft}
				onCloseConfigure={() => model.setConfigureIndex(null)}
				onSaveConfigure={model.saveCustomization}
			/>
		</div>
	);
}
