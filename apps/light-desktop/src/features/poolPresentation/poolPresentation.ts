import {
	DEFAULT_POOL_COLOR_PALETTE,
	type PoolColorMode,
	type PoolColorPalette,
	type PoolObjectType,
	type PoolPresentationInput,
	type PoolPresetFamily,
	resolvePoolPresentation,
} from "@tosklight/ui/pools";
import { useMemo } from "react";
import type { PoolPresentationConfiguration } from "../../api/types";
import { useConfigurationActions } from "../configuration/ConfigurationActionsProvider";
import { useDeskConfiguration } from "../configuration/ConfigurationState";
import { useActiveShowId } from "../deskSnapshot/DeskSnapshotState";

export function defaultPoolPresentation(): PoolPresentationConfiguration {
	return {
		palette: {
			group: DEFAULT_POOL_COLOR_PALETTE.group,
			macro_color: DEFAULT_POOL_COLOR_PALETTE.macro,
			dynamic: DEFAULT_POOL_COLOR_PALETTE.dynamic,
			cuelist: DEFAULT_POOL_COLOR_PALETTE.cuelist,
			sequence: DEFAULT_POOL_COLOR_PALETTE.sequence,
			preset: { ...DEFAULT_POOL_COLOR_PALETTE.preset },
		},
		modes: {},
		items: {},
	};
}

export function poolSurfaceKey(
	showId: string,
	objectType: PoolObjectType,
	paneId?: string,
): string {
	return paneId
		? `show:${showId}:pane:${paneId}`
		: `show:${showId}:builtin:${objectType}`;
}

export function poolItemKey(
	showId: string,
	objectType: PoolObjectType,
	objectId: string,
): string {
	return `show:${showId}:${objectType}:${objectId}`;
}

export function resetPoolColor(
	configuration: PoolPresentationConfiguration,
	objectType: PoolObjectType,
	presetFamily?: PoolPresetFamily,
): PoolPresentationConfiguration {
	if (objectType === "preset") {
		return {
			...configuration,
			palette: {
				...configuration.palette,
				preset: presetFamily
					? {
							...configuration.palette.preset,
							[presetFamily]: DEFAULT_POOL_COLOR_PALETTE.preset[presetFamily],
						}
					: { ...DEFAULT_POOL_COLOR_PALETTE.preset },
			},
		};
	}
	const field = objectType === "macro" ? "macro_color" : objectType;
	return {
		...configuration,
		palette: {
			...configuration.palette,
			[field]: DEFAULT_POOL_COLOR_PALETTE[objectType],
		},
	};
}

export function resetAllPoolColors(
	configuration: PoolPresentationConfiguration,
): PoolPresentationConfiguration {
	return {
		...configuration,
		palette: defaultPoolPresentation().palette,
	};
}

type ConfiguredInput = Omit<
	PoolPresentationInput,
	"palette" | "mode" | "itemColor"
> & {
	surfaceKey: string;
	showId: string;
	itemColorKey?: string;
	itemColor?: string | null;
	fallbackMode?: PoolColorMode;
};

export function resolveConfiguredPoolPresentation(
	configuration: PoolPresentationConfiguration,
	input: ConfiguredInput,
) {
	const item = input.itemColorKey
		? configuration.items[
				poolItemKey(input.showId, input.objectType, input.itemColorKey)
			]
		: undefined;
	return resolvePoolPresentation({
		objectType: input.objectType,
		presetFamily: input.presetFamily,
		states: input.states,
		mode: configuration.modes[input.surfaceKey] ?? input.fallbackMode ?? "type",
		palette: uiPalette(configuration),
		itemColor: item?.color ?? input.itemColor,
	});
}

export function usePoolPresentationConfiguration() {
	const desk = useDeskConfiguration();
	return useMemo(
		() => desk?.pool_presentation ?? defaultPoolPresentation(),
		[desk?.pool_presentation],
	);
}

export function usePoolPresentationSettings() {
	const configuration = usePoolPresentationConfiguration();
	const actions = useConfigurationActions();
	const showId = useActiveShowId();
	const save = (next: PoolPresentationConfiguration) =>
		actions?.setPoolPresentation(next) ?? Promise.resolve();
	return {
		configuration,
		showId,
		setMode(surfaceKey: string, mode: PoolColorMode) {
			return save({
				...configuration,
				modes: { ...configuration.modes, [surfaceKey]: mode },
			});
		},
		setTypeColor(objectType: Exclude<PoolObjectType, "preset">, color: string) {
			const field = objectType === "macro" ? "macro_color" : objectType;
			return save({
				...configuration,
				palette: { ...configuration.palette, [field]: color },
			});
		},
		setPresetColor(family: PoolPresetFamily, color: string) {
			return save({
				...configuration,
				palette: {
					...configuration.palette,
					preset: { ...configuration.palette.preset, [family]: color },
				},
			});
		},
		setItem(
			objectType: PoolObjectType,
			objectId: string,
			value: PoolPresentationConfiguration["items"][string],
		) {
			return save({
				...configuration,
				items: {
					...configuration.items,
					[poolItemKey(showId ?? "unresolved", objectType, objectId)]: value,
				},
			});
		},
		resetColor(objectType: PoolObjectType, presetFamily?: PoolPresetFamily) {
			return save(resetPoolColor(configuration, objectType, presetFamily));
		},
		resetAll() {
			return save(resetAllPoolColors(configuration));
		},
	};
}

function uiPalette(
	configuration: PoolPresentationConfiguration,
): PoolColorPalette {
	return {
		group: configuration.palette.group,
		macro: configuration.palette.macro_color,
		dynamic: configuration.palette.dynamic,
		cuelist: configuration.palette.cuelist,
		sequence: configuration.palette.sequence,
		preset: configuration.palette.preset,
	};
}
