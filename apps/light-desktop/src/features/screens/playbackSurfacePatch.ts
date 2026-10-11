import type { PlaybackSurfaceConfiguration } from "../../api/playbackConfigurationModels";
/** Configuration intents touch only changed fields of one named surface. */
export function playbackSurfacePatch(
	current: PlaybackSurfaceConfiguration | undefined,
	next: PlaybackSurfaceConfiguration,
): Partial<PlaybackSurfaceConfiguration> {
	if (!current) return { ...next };
	return {
		...(current.name !== next.name ? { name: next.name } : {}),
		...(JSON.stringify(current.layout) !== JSON.stringify(next.layout)
			? { layout: next.layout }
			: {}),
		...(JSON.stringify(current.page_source) !== JSON.stringify(next.page_source)
			? { page_source: next.page_source }
			: {}),
	};
}
