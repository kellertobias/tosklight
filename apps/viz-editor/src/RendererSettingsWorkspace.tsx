import { HorizontalFaderField, SelectField, SwitchField } from "@tosklight/ui";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { documentSession, type RendererSettings } from "./document/session";

export function RendererSettingsWorkspace({
	page,
	onError,
}: {
	page: "rendering" | "atmosphere" | "picture" | "features";
	onError: (reason: unknown) => void;
}) {
	const [draft, setDraft] = useState<RendererSettings | null>(null);
	const [status, setStatus] = useState("");
	const saveQueue = useRef<Promise<void>>(Promise.resolve());
	const saving = useRef(0);

	useEffect(() => {
		documentSession
			.rendererSettings()
			.then((settings) => {
				setDraft(settings);
			})
			.catch(onError);
	}, [onError]);

	useEffect(() => {
		let active = true;
		const refresh = window.setInterval(() => {
			if (saving.current) return;
			documentSession
				.rendererSettings()
				.then((settings) => {
					if (!active) return;
					setDraft((current) =>
						current && settingsMatch(current, settings) ? current : settings,
					);
				})
				.catch(onError);
		}, 75);
		return () => {
			active = false;
			window.clearInterval(refresh);
		};
	}, [onError]);

	if (!draft) {
		return (
			<section className="viz-renderer-settings">
				<p className="viz-renderer-settings-loading">
					Loading Visualizer settings…
				</p>
			</section>
		);
	}

	const update = (change: Partial<RendererSettings>) => {
		const next = { ...draft, ...change };
		setDraft(next);
		setStatus("Saving Visualizer settings…");
		saving.current += 1;
		saveQueue.current = saveQueue.current.then(async () => {
			try {
				const stored = await documentSession.saveRendererSettings(next);
				setDraft((current) => (current === next ? stored : current));
				setStatus("Visualizer settings applied.");
			} catch (reason) {
				setStatus("");
				onError(reason);
			} finally {
				saving.current -= 1;
			}
		});
	};

	return (
		<section className="viz-renderer-settings">
			<div className="viz-renderer-settings-scroll">
				<header>
					<h1>Visualizer</h1>
					<p>
						These controls update every Visualizer connected to this Editor live.
						Its source is the current document; live values still arrive through
						the Show screen’s DMX inputs.
					</p>
				</header>
				<div className="viz-renderer-settings-grid">
					{page === "rendering" ? <SettingsGroup title="Rendering">
						<SelectField
							label="Render quality"
							value={draft.quality ?? "follow"}
							onChange={(quality) =>
								update({
									quality:
										quality === "follow"
											? null
											: (quality as NonNullable<RendererSettings["quality"]>),
								})
							}
							options={[
								{ value: "follow", label: "Follow source" },
								{ value: "draft", label: "Draft" },
								{ value: "standard", label: "Standard" },
								{ value: "high", label: "High" },
								{ value: "ultra", label: "Ultra" },
							]}
						/>
						<SelectField
							label="Appearance"
							value={draft.theme}
							onChange={(theme) => update({ theme })}
							options={[
								{ value: "light_on_dark", label: "Light on dark" },
								{ value: "dark_on_light", label: "Dark on light" },
							]}
						/>
						<NumberSetting
							label="Environment brightness"
							value={draft.ambient}
							min={0}
							max={1}
							step={0.01}
							onChange={(ambient) => update({ ambient })}
							format={percent}
						/>
						<NumberSetting
							label="Exposure"
							value={draft.exposure}
							min={0.05}
							max={4}
							step={0.05}
							onChange={(exposure) => update({ exposure })}
							format={(value) => `${value.toFixed(2)}×`}
						/>
					</SettingsGroup> : null}

					{page === "atmosphere" ? <SettingsGroup title="Atmosphere">
						<NumberSetting
							label="Fog amount"
							value={draft.fog}
							min={0}
							max={1}
							step={0.01}
							onChange={(fog) => update({ fog })}
							format={percent}
						/>
						<NumberSetting
							label="Lamp fog cloudiness"
							value={draft.lampFogCloudiness}
							min={0}
							max={1}
							step={0.01}
							onChange={(lampFogCloudiness) => update({ lampFogCloudiness })}
							format={percent}
						/>
						<NumberSetting
							label="Lamp fog turbulence"
							value={draft.lampFogTurbulence}
							min={0}
							max={1}
							step={0.01}
							onChange={(lampFogTurbulence) => update({ lampFogTurbulence })}
							format={percent}
						/>
						<NumberSetting
							label="Laser fog cloudiness"
							value={draft.laserFogCloudiness}
							min={0}
							max={1}
							step={0.01}
							onChange={(laserFogCloudiness) => update({ laserFogCloudiness })}
							format={percent}
						/>
						<NumberSetting
							label="Laser fog turbulence"
							value={draft.laserFogTurbulence}
							min={0}
							max={1}
							step={0.01}
							onChange={(laserFogTurbulence) => update({ laserFogTurbulence })}
							format={percent}
						/>
						<NumberSetting
							label="Laser brightness"
							value={draft.laserBrightness}
							min={0}
							max={4}
							step={0.05}
							onChange={(laserBrightness) => update({ laserBrightness })}
							format={percent}
						/>
					</SettingsGroup> : null}

					{page === "picture" ? <SettingsGroup title="Picture">
						<NumberSetting
							label="Persistence of vision"
							value={draft.persistence}
							min={0}
							max={1}
							step={0.01}
							onChange={(persistence) => update({ persistence })}
							format={(value) => `${value.toFixed(2)} s`}
						/>
						<NumberSetting
							label="Persistence falloff"
							value={draft.persistenceFalloff}
							min={1}
							max={8}
							step={0.1}
							onChange={(persistenceFalloff) => update({ persistenceFalloff })}
							format={(value) => `${value.toFixed(1)}×`}
						/>
						<NumberSetting
							label="Crowd amount"
							value={draft.crowdAmount}
							min={0}
							max={1}
							step={0.01}
							onChange={(crowdAmount) => update({ crowdAmount })}
							format={percent}
						/>
						<label className="viz-renderer-background">
							<span>Background color</span>
							<input
								type="color"
								disabled={draft.background == null}
								value={rgbToHex(draft.background ?? [0.03, 0.04, 0.05])}
								onChange={(event) =>
									update({ background: hexToRgb(event.target.value) })
								}
							/>
						</label>
						<SwitchField
							label="Use custom background"
							offLabel={null}
							onLabel={null}
							checked={draft.background != null}
							onChange={(event) =>
								update({
									background: event.target.checked ? [0.03, 0.04, 0.05] : null,
								})
							}
						/>
					</SettingsGroup> : null}

					{page === "features" ? <SettingsGroup title="Features">
						<SwitchField
							label="Fixture / plan labels"
							offLabel={null}
							onLabel={null}
							checked={draft.showLabels}
							onChange={(event) => update({ showLabels: event.target.checked })}
						/>
						<SwitchField
							label="Show selection"
							offLabel={null}
							onLabel={null}
							checked={draft.showSelection}
							onChange={(event) =>
								update({ showSelection: event.target.checked })
							}
						/>
						<SelectField
							label="Floor grid"
							value={
								draft.floorGrid == null ? "follow" : String(draft.floorGrid)
							}
							onChange={(value) =>
								update({
									floorGrid: value === "follow" ? null : value === "true",
								})
							}
							options={[
								{ value: "follow", label: "Follow view" },
								{ value: "true", label: "Shown" },
								{ value: "false", label: "Hidden" },
							]}
						/>
						<label>
							<span>Blender path</span>
							<input
								value={draft.blender}
								placeholder="Find automatically"
								onChange={(event) => update({ blender: event.target.value })}
							/>
						</label>
					</SettingsGroup> : null}
				</div>
				{status ? (
					<output className="viz-editor-status">{status}</output>
				) : null}
			</div>
		</section>
	);
}

function SettingsGroup({
	title,
	children,
}: {
	title: string;
	children: ReactNode;
}) {
	return (
		<section>
			<h2>{title}</h2>
			{children}
		</section>
	);
}

function NumberSetting({
	label,
	value,
	min,
	max,
	step,
	onChange,
	format,
}: {
	label: string;
	value: number;
	min: number;
	max: number;
	step: number;
	onChange: (value: number) => void;
	format: (value: number) => string;
}) {
	return (
		<HorizontalFaderField
			label={label}
			value={value}
			minimum={min}
			maximum={max}
			step={step}
			display={format(value)}
			onChange={onChange}
		/>
	);
}

function percent(value: number) {
	return `${Math.round(value * 100)}%`;
}

function settingsMatch(left: RendererSettings, right: RendererSettings) {
	return JSON.stringify(left) === JSON.stringify(right);
}

function rgbToHex(rgb: [number, number, number]) {
	return `#${rgb
		.map((channel) =>
			Math.round(channel * 255)
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}

function hexToRgb(hex: string): [number, number, number] {
	return [1, 3, 5].map(
		(index) => Number.parseInt(hex.slice(index, index + 2), 16) / 255,
	) as [number, number, number];
}
