import { Button, ColorPickerField, FormLayout } from "@tosklight/ui";
import type {
	PoolColorMode,
	PoolObjectType,
	PoolPresetFamily,
} from "@tosklight/ui/pools";
import { useState } from "react";
import {
	poolSurfaceKey,
	usePoolPresentationSettings,
} from "../../features/poolPresentation/poolPresentation";

const LABELS: Record<PoolObjectType, string> = {
	group: "Groups",
	preset: "Presets",
	cuelist: "Cuelists",
	sequence: "Sequences",
	dynamic: "Dynamics",
	macro: "Macros",
};

const PRESET_LABELS: Record<PoolPresetFamily, string> = {
	mixed: "Mixed Presets",
	intensity: "Intensity Presets",
	color: "Color Presets",
	position: "Position Presets",
	beam: "Beam Presets",
};

export function PoolColorSettings({
	objectType,
	paneId,
	legacyPresetColors,
}: {
	objectType: PoolObjectType;
	paneId?: string;
	presetFamily?: PoolPresetFamily;
	legacyPresetColors?: boolean;
}) {
	const settings = usePoolPresentationSettings();
	const surfaceKey = poolSurfaceKey(
		settings.showId ?? "unresolved",
		objectType,
		paneId,
	);
	const mode =
		settings.configuration.modes[surfaceKey] ??
		(legacyPresetColors === false ? "individual" : "type");
	const [saving, setSaving] = useState(false);
	const persist = async (operation: () => Promise<void>) => {
		setSaving(true);
		try {
			await operation();
		} finally {
			setSaving(false);
		}
	};
	const setMode = (value: PoolColorMode) =>
		void persist(() => settings.setMode(surfaceKey, value));
	return (
		<section
			className="pool-color-settings"
			aria-label={`${LABELS[objectType]} colors`}
		>
			<h3>{LABELS[objectType]} colors</h3>
			<fieldset className="button-group">
				<legend>Pool color mode</legend>
				<Button
					className={mode === "type" ? "active" : ""}
					disabled={saving}
					aria-pressed={mode === "type"}
					onClick={() => setMode("type")}
				>
					Type colors
				</Button>
				<Button
					className={mode === "individual" ? "active" : ""}
					disabled={saving}
					aria-pressed={mode === "individual"}
					onClick={() => setMode("individual")}
				>
					Individual colors
				</Button>
			</fieldset>
			<p>
				Individual colors show only explicit item colors; uncolored items remain
				grey.
			</p>
			{saving && <p role="status">Saving pool colors…</p>}
		</section>
	);
}

export function PoolPaletteSettings() {
	const settings = usePoolPresentationSettings();
	const [saving, setSaving] = useState(false);
	const persist = async (operation: () => Promise<void>) => {
		setSaving(true);
		try {
			await operation();
		} finally {
			setSaving(false);
		}
	};
	const palette = settings.configuration.palette;
	return (
		<section className="pool-palette-settings" aria-label="Pool color defaults">
			<h3>Pool color defaults</h3>
			<p>
				These server-wide presentation colors apply to every desktop and do not
				change portable show objects.
			</p>
			<FormLayout labelPlacement="side">
				{(
					[
						["group", "Groups", palette.group],
						["macro", "Macros", palette.macro_color],
						["dynamic", "Dynamics", palette.dynamic],
						["cuelist", "Cuelists", palette.cuelist],
						["sequence", "Sequences", palette.sequence],
					] as const
				).map(([objectType, label, color]) => (
					<ColorPickerField
						key={objectType}
						label={label}
						value={color}
						disabled={saving}
						onChange={(next) =>
							void persist(() => settings.setTypeColor(objectType, next))
						}
					/>
				))}
				{(Object.keys(PRESET_LABELS) as PoolPresetFamily[]).map((family) => (
					<ColorPickerField
						key={family}
						label={PRESET_LABELS[family]}
						value={palette.preset[family]}
						disabled={saving}
						onChange={(next) =>
							void persist(() => settings.setPresetColor(family, next))
						}
					/>
				))}
			</FormLayout>
			<Button disabled={saving} onClick={() => void persist(settings.resetAll)}>
				Reset all pool colors
			</Button>
			{saving && <p role="status">Saving pool color defaults…</p>}
		</section>
	);
}
