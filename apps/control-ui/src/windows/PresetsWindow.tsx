import { useState } from "react";
import { useServer } from "../api/ServerContext";
import { useCommandLineSurface } from "../components/control/commandLine/useCommandLineSurface";
import { requestUpdateTarget } from "../components/control/updateWorkflow";
import { GroupStrip } from "../components/shared/GroupStrip";
import type { RecordMode } from "../components/shared/RecordModeDialog";
import { presets } from "../data/mockData";
import type { PresetRecallActions } from "../features/presetRecall/contracts";
import { usePresetRecall } from "../features/presetRecall/PresetRecallProvider";
import { usePresetRecording } from "../features/presetRecording/PresetRecordingProvider";
import {
	type PresetCard,
	resolvePresetCards,
} from "../features/presetRecording/presetCards";
import { submitPresetRecording } from "../features/presetRecording/submitRecording";
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

function loadPresetCustomizations() {
	try {
		return JSON.parse(
			localStorage.getItem("light.preset-button-customizations") ?? "{}",
		) as Record<string, PresetCustomization>;
	} catch {
		return {};
	}
}

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
			color: saved.color ?? preset?.body.color ?? "#d98236",
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

export function PresetsWindow({
	active = true,
	compact,
	paneId,
	showGroupShortcuts,
	presetFamily,
	presetPoolColors,
}: WindowProps) {
	const server = useServer();
	const presetRecall = usePresetRecall(active);
	const selection = presetRecall.selection;
	const storedPresets = usePresets(active);
	const presetRecording = usePresetRecording();
	const command = useCommandLineSurface({
		enabled: active,
		observeCommand: false,
	});
	const { state, dispatch } = useApp();
	const family = compact
		? (presetFamily ?? state.presetFamily)
		: state.presetFamily;
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
	const colorsEnabled = compact
		? (presetPoolColors ?? true)
		: state.presetPoolColors;
	const [customizations, setCustomizations] = useState<
		Record<string, PresetCustomization>
	>(loadPresetCustomizations);
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
	const fallback = fallbackPresets(!server.bootstrap);
	const stored = server.bootstrap?.active_show ? storedPresets : fallback;
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
			preloadActive: state.preload !== "idle",
			actions: presetRecording,
			storePreload: server.storePreload,
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

	return (
		<div
			className={`pool-window preset-pool-window ${colorsEnabled ? "pool-colors" : "pool-colors-disabled"} pool-family-${family.toLowerCase()}`}
		>
			{!compact && (
				<PresetWindowHeader
					family={family}
					onFamily={setFamily}
					onOpenGroups={() =>
						dispatch({ type: "OPEN_BUILTIN", kind: "groups" })
					}
					onSettings={setSettingsAnchor}
				/>
			)}
			<PresetCardGrid
				cards={cards}
				family={family}
				customizations={customizations}
				colorsEnabled={colorsEnabled}
				selectionCount={selection?.selected.length ?? 0}
				storeArmed={state.storeArmed}
				updateArmed={state.updateArmed}
				setArmed={state.presetSetArmed}
				onActivate={activate}
			/>
			{groupsVisible && <GroupStrip active={active} />}
			<PresetWindowOverlays
				settingsAnchor={settingsAnchor}
				family={family}
				colorsEnabled={colorsEnabled}
				cards={cards}
				recordIndex={recordPresetIndex}
				configureIndex={configureIndex}
				configureDraft={configureDraft}
				onFamily={setFamily}
				onColors={(value) =>
					dispatch(
						compact && paneId
							? { type: "SET_PANE_PRESET_COLORS", id: paneId, value }
							: { type: "SET_PRESET_POOL_COLORS", value },
					)
				}
				onCloseSettings={() => setSettingsAnchor(null)}
				onRecord={recordPreset}
				onCancelRecord={cancelRecording}
				onDraft={setConfigureDraft}
				onCloseConfigure={() => setConfigureIndex(null)}
				onSaveConfigure={() => {
					if (configureIndex == null) return;
					const id =
						cards[configureIndex]?.id ??
						presetStorageKey(presetAddress(family, configureIndex + 1));
					const next = { ...customizations, [id]: configureDraft };
					setCustomizations(next);
					localStorage.setItem(
						"light.preset-button-customizations",
						JSON.stringify(next),
					);
					setConfigureIndex(null);
				}}
			/>
		</div>
	);
}
