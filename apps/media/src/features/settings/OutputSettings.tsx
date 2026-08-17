// One output's stored identity: where it opens, how it presents, and which DMX block feeds it.
//
// None of these values are live controls. Saving stores the next output identity, and the server
// says explicitly that the running surface is left alone until the next start.

import {
	Button,
	CheckboxField,
	NumberField,
	SelectField,
} from "@tosklight/ui/controls";
import type { Dispatch, SetStateAction } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useFailureToast } from "../../app/ToastContext";
import { ApiFailure, api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	OutputConfigurationView,
	UpdateOutputConfiguration,
} from "../../shared/api/generated/media-wire";
import { SettingsSaveState } from "./SettingsSaveState";

interface ConfigurationResource {
	data: OutputConfigurationView | undefined;
	failure: ApiFailure | undefined;
	reload: () => void;
}

/** Reads one independently addressed output without adding a singleton output cache. */
function useOutputConfiguration(outputId: string): ConfigurationResource {
	const [data, setData] = useState<OutputConfigurationView>();
	const [failure, setFailure] = useState<ApiFailure>();
	const [revision, setRevision] = useState(0);

	useEffect(() => {
		let current = true;
		void api
			.outputConfiguration(outputId)
			.then((configuration) => {
				if (!current) return;
				setData(configuration);
				setFailure(undefined);
			})
			.catch((error: unknown) => {
				if (!current) return;
				setFailure(
					error instanceof ApiFailure
						? error
						: new ApiFailure("unexpected-error", String(error), 0),
				);
			});
		return () => {
			current = false;
		};
	}, [outputId, revision]);

	return {
		data,
		failure,
		reload: useCallback(() => setRevision((value) => value + 1), []),
	};
}

export function OutputSettings({
	outputId,
	outputName,
	mode = "all",
	direct = false,
}: {
	outputId: string;
	outputName: string;
	mode?: "all" | "picture" | "sound" | "dmx";
	direct?: boolean;
}) {
	const configuration = useOutputConfiguration(outputId);
	const editing = useEditing(configuration.reload);
	useFailureToast(editing.failure);

	if (configuration.failure && !configuration.data) {
		return (
			<article
				className="media-settings-section"
				aria-label={`${outputName} output settings`}
			>
				<p className="media-state is-error" role="alert">
					{configuration.failure.message}{" "}
					<Button size="compact" onClick={configuration.reload}>
						Try again
					</Button>
				</p>
			</article>
		);
	}

	if (!configuration.data) {
		return (
			<article
				className="media-settings-section"
				aria-label={`${outputName} output settings`}
			>
				<p className="media-state">Reading {outputName} output settings…</p>
			</article>
		);
	}

	const output = configuration.data;
	const pendingRestart = outputPendingRestart(output, mode);
	return (
		<article
			className="media-settings-section"
			aria-label={`${output.name} ${mode === "dmx" ? "DMX input" : "output"} settings`}
		>
			<div className="media-settings-section-heading">
				<h3>{output.name} output</h3>
				{direct && (
					<SettingsSaveState
						busy={editing.busy}
						failed={editing.failure !== undefined}
						restartBound
					/>
				)}
			</div>
			{direct || editing.editing === output.id ? (
				<OutputEditor
					key={outputEditorKey(output, mode)}
					output={output}
					mode={mode}
					busy={editing.busy}
					onCancel={editing.cancel}
					showActions={!direct}
					showCancel={!direct}
					onSave={(edit) =>
						void editing.save(() =>
							api.updateOutputConfiguration(output.id, edit),
						)
					}
				/>
			) : (
				<>
					<OutputFacts output={output} mode={mode} />
					{output.takesEffectOnRestart && <RestartNotice />}
					<div className="media-settings-actions">
						<Button onClick={() => editing.begin(output.id)}>
							{mode === "dmx" ? "Change DMX input" : "Change output settings"}
						</Button>
					</div>
				</>
			)}
			{direct && pendingRestart && (
				<>
					<RestartNotice />
					<div className="media-settings-actions">
						<Button
							onClick={() =>
								void editing.save(() =>
									api.updateOutputConfiguration(
										output.id,
										revertOutputEdit(output, mode),
									),
								)
							}
						>
							Revert to current settings
						</Button>
					</div>
				</>
			)}
		</article>
	);
}

function OutputFacts({
	output,
	mode,
}: {
	output: OutputConfigurationView;
	mode: "all" | "picture" | "sound" | "dmx";
}) {
	return (
		<dl className="media-facts">
			{mode !== "dmx" && mode !== "sound" && (
				<>
					<dt>Target</dt>
					<dd>{describeTarget(output)}</dd>
					<dt>Resolution</dt>
					<dd>
						{output.width} × {output.height}
					</dd>
					<dt>Presentation</dt>
					<dd>{describePresentation(output)}</dd>
					<dt>Sound output</dt>
					<dd>{describeSoundOutput(output)}</dd>
				</>
			)}
			{mode !== "picture" && mode !== "sound" && (
				<>
					<dt>Personality</dt>
					<dd>
						{output.personality === "two-layers" ? "2 layers" : "8 layers"}
					</dd>
					<dt>DMX input</dt>
					<dd>
						{output.protocol === "art-net" ? "Art-Net" : "sACN"}, universe{" "}
						{output.universe}, address {output.startAddress}
					</dd>
				</>
			)}
		</dl>
	);
}

function describeTarget(output: OutputConfigurationView): string {
	if (output.targetKind === "off-screen") return "Off-screen (no window)";
	const monitor =
		output.monitorBy === "name"
			? `monitor named ${output.monitorValue ?? "not set"}`
			: `monitor number ${output.monitorValue ?? "not set"}`;
	return `${monitor}${output.fullscreen ? ", full-screen" : ", in a window"}`;
}

function describePresentation(output: OutputConfigurationView): string {
	if (output.presentation === "display-synchronized")
		return "Display synchronized";
	if (output.presentation === "fixed-fps") {
		return `Fixed at ${output.framesPerSecond ?? "?"} frames per second`;
	}
	return "Unlocked (diagnostic)";
}

function describeSoundOutput(output: OutputConfigurationView): string {
	if (output.soundOutputKind === "disabled") return "Muted";
	if (output.soundOutputKind === "system-default") return "System default";
	return output.soundOutputName ?? "Device not set";
}

function RestartNotice() {
	return (
		<p className="media-state is-notice">
			Saved output changes take effect the next time this server starts. The
			output running now stays as it is.
		</p>
	);
}

type ResolutionMode = "monitor" | "720p" | "1080p" | "480p" | "manual";

const FRAME_RATE_OPTIONS = [
	{ value: "23.976", label: "23.976 fps · NTSC film" },
	{ value: "24", label: "24 fps" },
	{ value: "25", label: "25 fps · PAL" },
	{ value: "29.97", label: "29.97 fps · NTSC" },
	{ value: "30", label: "30 fps" },
	{ value: "40", label: "40 fps" },
	{ value: "44", label: "44 fps" },
	{ value: "50", label: "50 fps · PAL" },
	{ value: "59.94", label: "59.94 fps · NTSC" },
	{ value: "60", label: "60 fps" },
];

function resolutionDimensions(
	mode: ResolutionMode,
	monitor: OutputConfigurationView["availableMonitors"][number] | undefined,
): { width: number; height: number } | undefined {
	if (mode === "monitor") return monitor;
	if (mode === "720p") return { width: 1280, height: 720 };
	if (mode === "1080p") return { width: 1920, height: 1080 };
	if (mode === "480p") return { width: 720, height: 480 };
	return undefined;
}

function resolutionModeFor(
	width: number,
	height: number,
	monitor: OutputConfigurationView["availableMonitors"][number] | undefined,
): ResolutionMode {
	if (monitor?.width === width && monitor.height === height) return "monitor";
	if (width === 1280 && height === 720) return "720p";
	if (width === 1920 && height === 1080) return "1080p";
	if (width === 720 && height === 480) return "480p";
	return "manual";
}

function PictureFields({
	resolutionMode,
	canTakeFromMonitor,
	width,
	height,
	presentation,
	framesPerSecond,
	setWidth,
	setHeight,
	setPresentation,
	setFramesPerSecond,
	setResolutionMode,
}: {
	resolutionMode: ResolutionMode;
	canTakeFromMonitor: boolean;
	width: number;
	height: number;
	presentation: OutputConfigurationView["presentation"];
	framesPerSecond: number;
	setWidth: Dispatch<SetStateAction<number>>;
	setHeight: Dispatch<SetStateAction<number>>;
	setPresentation: Dispatch<
		SetStateAction<OutputConfigurationView["presentation"]>
	>;
	setFramesPerSecond: Dispatch<SetStateAction<number>>;
	setResolutionMode: (mode: ResolutionMode) => void;
}) {
	return (
		<fieldset>
			<legend>Picture</legend>
			<SelectField
				label="Resolution"
				value={resolutionMode}
				options={[
					{
						value: "monitor",
						label: "Take from monitor",
						disabled: !canTakeFromMonitor,
					},
					{ value: "720p", label: "720p · 1280 × 720" },
					{ value: "1080p", label: "1080p · 1920 × 1080" },
					{ value: "480p", label: "480p · 720 × 480" },
					{ value: "manual", label: "Manual" },
				]}
				onChange={setResolutionMode}
			/>
			{resolutionMode === "manual" && (
				<>
					<NumberField
						label="Width"
						description="Pixels. This is the render width, including on a full-screen monitor."
						min={1}
						step={1}
						value={String(width)}
						onChange={(event) => setWidth(Number(event.target.value))}
					/>
					<NumberField
						label="Height"
						description="Pixels. This is the render height, including on a full-screen monitor."
						min={1}
						step={1}
						value={String(height)}
						onChange={(event) => setHeight(Number(event.target.value))}
					/>
				</>
			)}
			<SelectField
				label="Frame rate"
				value={presentation}
				options={[
					{ value: "display-synchronized", label: "Display synchronized" },
					{ value: "fixed-fps", label: "Fixed frame rate" },
					{ value: "unlocked", label: "Unlocked" },
				]}
				onChange={setPresentation}
			/>
			{presentation === "fixed-fps" && (
				<SelectField
					label="Fixed frame rate"
					value={String(framesPerSecond)}
					options={FRAME_RATE_OPTIONS}
					onChange={(value) => setFramesPerSecond(Number(value))}
				/>
			)}
		</fieldset>
	);
}

function DmxInputFields({
	personality,
	personalityLayout,
	protocol,
	universe,
	startAddress,
	setPersonality,
	setPersonalityLayout,
	setProtocol,
	setUniverse,
	setStartAddress,
}: {
	personality: OutputConfigurationView["personality"];
	personalityLayout: OutputConfigurationView["personalityLayout"];
	protocol: OutputConfigurationView["protocol"];
	universe: number;
	startAddress: number;
	setPersonality: Dispatch<
		SetStateAction<OutputConfigurationView["personality"]>
	>;
	setPersonalityLayout: Dispatch<
		SetStateAction<OutputConfigurationView["personalityLayout"]>
	>;
	setProtocol: Dispatch<SetStateAction<OutputConfigurationView["protocol"]>>;
	setUniverse: Dispatch<SetStateAction<number>>;
	setStartAddress: Dispatch<SetStateAction<number>>;
}) {
	const slotsPerLayer = personalityLayout === "legacy" ? 34 : 39;
	const masterSlots = personalityLayout === "legacy" ? 7 : 11;
	const footprint =
		(personality === "two-layers" ? 2 : 8) * slotsPerLayer + masterSlots;
	const highestStartAddress = 513 - footprint;
	return (
		<fieldset>
			<legend>DMX input</legend>
			<SelectField
				label="Personality"
				value={personality}
				options={[
					{ value: "two-layers", label: "2 layers (89 slots)" },
					{ value: "eight-layers", label: "8 layers (323 slots)" },
				]}
				onChange={setPersonality}
			/>
			<SelectField
				label="Channel layout"
				value={personalityLayout}
				options={[
					{ value: "legacy", label: "Legacy (existing desk patches)" },
					{ value: "current", label: "Current (mask positioning)" },
				]}
				onChange={setPersonalityLayout}
			/>
			<SelectField
				label="Protocol"
				value={protocol}
				options={[
					{ value: "art-net", label: "Art-Net" },
					{ value: "sacn", label: "sACN" },
				]}
				onChange={setProtocol}
			/>
			<NumberField
				label="Universe"
				min={protocol === "sacn" ? 1 : 0}
				max={protocol === "sacn" ? 63_999 : 32_767}
				step={1}
				value={String(universe)}
				onChange={(event) => setUniverse(Number(event.target.value))}
			/>
			<NumberField
				label="Start address"
				description={`1 to ${highestStartAddress}; the complete ${footprint}-slot personality must fit in one universe.`}
				min={1}
				max={highestStartAddress}
				step={1}
				value={String(startAddress)}
				onChange={(event) => setStartAddress(Number(event.target.value))}
			/>
		</fieldset>
	);
}

function SoundOutputFields({
	kind,
	name,
	availableDevices,
	setKind,
	setName,
}: {
	kind: OutputConfigurationView["soundOutputKind"];
	name: string;
	availableDevices: string[];
	setKind: Dispatch<SetStateAction<OutputConfigurationView["soundOutputKind"]>>;
	setName: Dispatch<SetStateAction<string>>;
}) {
	return (
		<fieldset>
			<legend>Sound output</legend>
			<SelectField
				label="Sound output"
				value={kind}
				options={[
					{ value: "disabled", label: "Muted" },
					{ value: "system-default", label: "System default" },
					{ value: "device", label: "Named device" },
				]}
				onChange={(next) => {
					setKind(next);
					if (next === "device" && !name && availableDevices[0]) {
						setName(availableDevices[0]);
					}
				}}
			/>
			{kind === "device" && (
				<SelectField
					label="Device"
					value={name}
					options={deviceOptions(availableDevices, name)}
					onChange={setName}
				/>
			)}
		</fieldset>
	);
}

export function OutputEditor({
	output,
	busy,
	onSave,
	onCancel,
	mode = "all",
	showActions = true,
	showCancel = true,
}: {
	output: OutputConfigurationView;
	busy: boolean;
	onSave: (edit: UpdateOutputConfiguration) => void;
	onCancel: () => void;
	mode?: "all" | "picture" | "sound" | "dmx";
	showActions?: boolean;
	showCancel?: boolean;
}) {
	const [targetKind, setTargetKind] = useState(output.targetKind);
	const [monitorBy, setMonitorBy] = useState(output.monitorBy ?? "index");
	const [monitorValue, setMonitorValue] = useState(output.monitorValue ?? "0");
	const [fullscreen, setFullscreen] = useState(output.fullscreen);
	const [width, setWidth] = useState(output.width);
	const [height, setHeight] = useState(output.height);
	const selectedMonitor = resolveMonitor(output, monitorBy, monitorValue);
	const [resolutionMode, setResolutionModeState] = useState<ResolutionMode>(
		() => resolutionModeFor(output.width, output.height, selectedMonitor),
	);
	const [presentation, setPresentation] = useState(output.presentation);
	const [framesPerSecond, setFramesPerSecond] = useState(
		output.framesPerSecond ?? 60,
	);
	const [soundOutputKind, setSoundOutputKind] = useState(
		output.soundOutputKind,
	);
	const [soundOutputName, setSoundOutputName] = useState(
		output.soundOutputName ?? "",
	);
	const [personality, setPersonality] = useState(output.personality);
	const [personalityLayout, setPersonalityLayout] = useState(
		output.personalityLayout,
	);
	const [protocol, setProtocol] = useState(output.protocol);
	const [universe, setUniverse] = useState(output.universe);
	const [startAddress, setStartAddress] = useState(output.startAddress);
	const setResolutionMode = (mode: ResolutionMode) => {
		setResolutionModeState(mode);
		const dimensions = resolutionDimensions(mode, selectedMonitor);
		if (dimensions) {
			setWidth(dimensions.width);
			setHeight(dimensions.height);
		}
	};
	const form = useRef<HTMLFormElement>(null);
	const mounted = useRef(false);
	useEffect(() => {
		if (showActions) return;
		if (!mounted.current) {
			mounted.current = true;
			return;
		}
		const timer = window.setTimeout(() => form.current?.requestSubmit(), 350);
		return () => window.clearTimeout(timer);
	}, [
		showActions,
		targetKind,
		monitorBy,
		monitorValue,
		fullscreen,
		width,
		height,
		presentation,
		framesPerSecond,
		soundOutputKind,
		soundOutputName,
		personality,
		personalityLayout,
		protocol,
		universe,
		startAddress,
	]);

	return (
		<form
			ref={form}
			data-media-settings-form={mode}
			className="media-settings-form"
			onSubmit={(event) => {
				event.preventDefault();
				const edit: UpdateOutputConfiguration = { requestId: requestId() };
				if (mode === "all" || mode === "picture") {
					Object.assign(edit, {
						targetKind,
						...(targetKind === "monitor"
							? { monitorBy, monitorValue: monitorValue.trim(), fullscreen }
							: {}),
						width,
						height,
						presentation,
						...(presentation === "fixed-fps" ? { framesPerSecond } : {}),
					});
				}
				if (mode === "all" || mode === "sound") {
					Object.assign(edit, {
						soundOutputKind,
						...(soundOutputKind === "device"
							? { soundOutputName: soundOutputName.trim() }
							: {}),
					});
				}
				if (mode === "all" || mode === "dmx") {
					Object.assign(edit, {
						personality,
						personalityLayout,
						protocol,
						universe,
						startAddress,
					});
				}
				onSave(edit);
			}}
		>
			{mode !== "dmx" && mode !== "sound" && (
				<fieldset>
					<legend>Output target</legend>
					<SelectField
						label="Target"
						value={targetKind}
						options={[
							{ value: "off-screen", label: "Off-screen (no window)" },
							{ value: "monitor", label: "Monitor" },
						]}
						onChange={setTargetKind}
					/>
					{targetKind === "monitor" && (
						<>
							<SelectField
								label="Monitor"
								value={`${monitorBy}:${monitorValue}`}
								options={monitorOptions(output, monitorBy, monitorValue)}
								onChange={(selection) => {
									const [by, ...value] = selection.split(":");
									const nextValue = value.join(":");
									setMonitorBy(by);
									setMonitorValue(nextValue);
									if (resolutionMode === "monitor") {
										const monitor = resolveMonitor(output, by, nextValue);
										if (monitor) {
											setWidth(monitor.width);
											setHeight(monitor.height);
										}
									}
								}}
							/>
							<CheckboxField
								label="Full-screen"
								stateLabel="Use the entire monitor"
								checked={fullscreen}
								onChange={(event) => setFullscreen(event.target.checked)}
							/>
						</>
					)}
				</fieldset>
			)}

			{mode !== "dmx" && mode !== "sound" && (
				<PictureFields
					{...{
						resolutionMode,
						canTakeFromMonitor: selectedMonitor !== undefined,
						width,
						height,
						presentation,
						framesPerSecond,
						setWidth,
						setHeight,
						setPresentation,
						setFramesPerSecond,
						setResolutionMode,
					}}
				/>
			)}

			{(mode === "all" || mode === "sound") && (
				<SoundOutputFields
					kind={soundOutputKind}
					name={soundOutputName}
					availableDevices={output.availableSoundOutputs}
					setKind={setSoundOutputKind}
					setName={setSoundOutputName}
				/>
			)}

			{mode !== "picture" && mode !== "sound" && (
				<DmxInputFields
					{...{
						personality,
						personalityLayout,
						protocol,
						universe,
						startAddress,
						setPersonality,
						setPersonalityLayout,
						setProtocol,
						setUniverse,
						setStartAddress,
					}}
				/>
			)}

			{showActions && (
				<div className="media-settings-actions">
					<Button type="submit" variant="primary" loading={busy}>
						{mode === "dmx"
							? "Save DMX input"
							: mode === "sound"
								? "Save sound output"
								: "Save picture output"}
					</Button>
					{showCancel && <Button onClick={onCancel}>Cancel</Button>}
				</div>
			)}
		</form>
	);
}

function outputPendingRestart(
	output: OutputConfigurationView,
	mode: "all" | "picture" | "sound" | "dmx",
): boolean {
	if (mode === "picture") return output.picturePendingRestart;
	if (mode === "sound") return output.soundPendingRestart;
	if (mode === "dmx") return output.dmxPendingRestart;
	return (
		output.picturePendingRestart ||
		output.soundPendingRestart ||
		output.dmxPendingRestart
	);
}

function revertOutputEdit(
	output: OutputConfigurationView,
	mode: "all" | "picture" | "sound" | "dmx",
): UpdateOutputConfiguration {
	const active = output.active;
	const edit: UpdateOutputConfiguration = { requestId: requestId() };
	if (mode === "all" || mode === "picture") {
		Object.assign(edit, {
			targetKind: active.targetKind,
			...(active.targetKind === "monitor"
				? {
						monitorBy: active.monitorBy,
						monitorValue: active.monitorValue,
						fullscreen: active.fullscreen,
					}
				: {}),
			width: active.width,
			height: active.height,
			presentation: active.presentation,
			...(active.presentation === "fixed-fps"
				? { framesPerSecond: active.framesPerSecond }
				: {}),
		});
	}
	if (mode === "all" || mode === "sound") {
		Object.assign(edit, {
			soundOutputKind: active.soundOutputKind,
			...(active.soundOutputKind === "device"
				? { soundOutputName: active.soundOutputName }
				: {}),
		});
	}
	if (mode === "all" || mode === "dmx") {
		Object.assign(edit, {
			personality: active.personality,
			personalityLayout: active.personalityLayout,
			protocol: active.protocol,
			universe: active.universe,
			startAddress: active.startAddress,
		});
	}
	return edit;
}

function outputEditorKey(
	output: OutputConfigurationView,
	mode: "all" | "picture" | "sound" | "dmx",
): string {
	return `${mode}:${output.targetKind}:${output.monitorBy}:${output.monitorValue}:${output.fullscreen}:${output.width}:${output.height}:${output.presentation}:${output.framesPerSecond}:${output.soundOutputKind}:${output.soundOutputName}:${output.personality}:${output.personalityLayout}:${output.protocol}:${output.universe}:${output.startAddress}`;
}

function monitorOptions(
	output: OutputConfigurationView,
	by: string,
	value: string,
) {
	const configured = `${by}:${value}`;
	const options = output.availableMonitors.map((monitor) => ({
		value: `index:${monitor.index}`,
		label: `Display ${monitor.index + 1} · ${monitor.name} · ${monitor.width} × ${monitor.height}`,
	}));
	if (!options.some((option) => option.value === configured)) {
		options.unshift({
			value: configured,
			label: `Configured monitor · ${value}`,
		});
	}
	return options;
}

function resolveMonitor(
	output: OutputConfigurationView,
	by: string,
	value: string,
) {
	if (by === "index") {
		const index = Number(value);
		return output.availableMonitors.find((monitor) => monitor.index === index);
	}
	if (by === "name") {
		return output.availableMonitors.find((monitor) => monitor.name === value);
	}
	return undefined;
}

function deviceOptions(devices: string[], selected: string) {
	const options = devices.map((device) => ({ value: device, label: device }));
	if (selected && !devices.includes(selected)) {
		options.unshift({ value: selected, label: `${selected} · unavailable` });
	}
	return options;
}
