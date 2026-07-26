import { useEffect, useRef } from "react";
import type { PoolPresentationConfiguration } from "../../api/types";
import { useConfigurationActions } from "../configuration/ConfigurationActionsProvider";
import { useDeskConfiguration } from "../configuration/ConfigurationState";
import { useActiveShowId } from "../deskSnapshot/DeskSnapshotState";
import { defaultPoolPresentation, poolItemKey } from "./poolPresentation";

const LEGACY_KEY = "light.preset-button-customizations";

/**
 * Moves the former browser-global Preset button customizations into the
 * authenticated desk store once a show scope is known.
 */
export function PoolPresentationLegacyMigration() {
	const showId = useActiveShowId();
	const desk = useDeskConfiguration();
	const actions = useConfigurationActions();
	const attempted = useRef<string | null>(null);
	useEffect(() => {
		if (!showId || !desk || !actions || attempted.current === showId) return;
		attempted.current = showId;
		const marker = `${LEGACY_KEY}:migrated:${showId}`;
		if (localStorage.getItem(marker) === "true") return;
		const legacy = readLegacyPresentations();
		if (Object.keys(legacy).length === 0) {
			localStorage.setItem(marker, "true");
			return;
		}
		const current = desk.pool_presentation ?? defaultPoolPresentation();
		void actions
			.setPoolPresentation(
				migrateLegacyPresetPresentations(current, showId, legacy),
			)
			.then(() => {
				localStorage.removeItem(LEGACY_KEY);
				localStorage.setItem(marker, "true");
			})
			.catch(() => {
				attempted.current = null;
			});
	}, [actions, desk, showId]);
	return null;
}

export function migrateLegacyPresetPresentations(
	current: PoolPresentationConfiguration,
	showId: string,
	legacy: PoolPresentationConfiguration["items"],
) {
	const items = { ...current.items };
	for (const [id, item] of Object.entries(legacy)) {
		const key = poolItemKey(showId, "preset", id);
		items[key] ??= item;
	}
	return { ...current, items };
}

function readLegacyPresentations(): PoolPresentationConfiguration["items"] {
	try {
		const value = JSON.parse(localStorage.getItem(LEGACY_KEY) ?? "{}");
		return value && typeof value === "object"
			? (value as PoolPresentationConfiguration["items"])
			: {};
	} catch {
		return {};
	}
}
