// One output's stored identity: where it opens, how it presents, and which DMX block feeds it.
//
// None of these values are live controls. Saving stores the next output identity, and the server
// says explicitly that the running surface is left alone until the next start.

import {
	Button,
	CheckboxField,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui/controls";
import type { Dispatch, SetStateAction } from "react";
import { useCallback, useEffect, useState } from "react";
import { ApiFailure, api } from "../../shared/api/client";
import { requestId, useEditing } from "../../shared/api/editing";
import type {
	OutputConfigurationView,
	UpdateOutputConfiguration,
} from "../../shared/api/generated/media-wire";

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
}: {
	outputId: string;
	outputName: string;
	mode?: "all" | "picture" | "dmx";
}) {
	const configuration = useOutputConfiguration(outputId);
	const editing = useEditing(configuration.reload);

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
	return (
		<article
			className="media-settings-section"
			aria-label={`${output.name} ${mode === "dmx" ? "DMX input" : "output"} settings`}
		>
			<h3>{output.name} output</h3>
			{editing.failure && (
				<p className="media-state is-error" role="alert">
					{editing.failure.message}{" "}
					<Button size="compact" onClick={editing.dismiss}>
						Dismiss
					</Button>
				</p>
			)}

			{editing.editing === output.id ? (
				<OutputEditor
					output={output}
					mode={mode}
					busy={editing.busy}
					onCancel={editing.cancel}
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
		</article>
	);
}

function OutputFacts({
	output,
	mode,
}: {
	output: OutputConfigurationView;
	mode: "all" | "picture" | "dmx";
}) {
	return (
		<dl className="media-facts">
			{mode !== "dmx" && (
				<>
					<dt>Target</dt>
					<dd>{describeTarget(output)}</dd>
					<dt>Resolution</dt>
					<dd>
						{output.width} × {output.height}
					</dd>
					<dt>Presentation</dt>
					<dd>{describePresentation(output)}</dd>
				</>
			)}
			{mode !== "picture" && (
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

function RestartNotice() {
	return (
		<p className="media-state is-notice">
			Saved output changes take effect the next time this server starts. The
			output running now stays as it is.
		</p>
	);
}

function PictureFields({
	width,
	height,
	presentation,
	framesPerSecond,
	setWidth,
	setHeight,
	setPresentation,
	setFramesPerSecond,
}: {
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
}) {
	return (
		<fieldset>
			<legend>Picture</legend>
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
			<SelectField
				label="Presentation"
				value={presentation}
				options={[
					{ value: "display-synchronized", label: "Display synchronized" },
					{ value: "fixed-fps", label: "Fixed frame rate" },
					{ value: "unlocked", label: "Unlocked (diagnostic)" },
				]}
				onChange={setPresentation}
			/>
			{presentation === "fixed-fps" && (
				<NumberField
					label="Frames per second"
					description="1 to 65535. Display synchronization is preferable for a monitor."
					min={1}
					max={65_535}
					step={1}
					value={String(framesPerSecond)}
					onChange={(event) => setFramesPerSecond(Number(event.target.value))}
				/>
			)}
		</fieldset>
	);
}

function DmxInputFields({
	personality,
	protocol,
	universe,
	startAddress,
	setPersonality,
	setProtocol,
	setUniverse,
	setStartAddress,
}: {
	personality: OutputConfigurationView["personality"];
	protocol: OutputConfigurationView["protocol"];
	universe: number;
	startAddress: number;
	setPersonality: Dispatch<
		SetStateAction<OutputConfigurationView["personality"]>
	>;
	setProtocol: Dispatch<SetStateAction<OutputConfigurationView["protocol"]>>;
	setUniverse: Dispatch<SetStateAction<number>>;
	setStartAddress: Dispatch<SetStateAction<number>>;
}) {
	const highestStartAddress = personality === "two-layers" ? 438 : 234;
	return (
		<fieldset>
			<legend>DMX input</legend>
			<SelectField
				label="Personality"
				value={personality}
				options={[
					{ value: "two-layers", label: "2 layers (75 slots)" },
					{ value: "eight-layers", label: "8 layers (279 slots)" },
				]}
				onChange={setPersonality}
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
				description={`1 to ${highestStartAddress}; the complete ${personality === "two-layers" ? "75" : "279"}-slot personality must fit in one universe.`}
				min={1}
				max={highestStartAddress}
				step={1}
				value={String(startAddress)}
				onChange={(event) => setStartAddress(Number(event.target.value))}
			/>
		</fieldset>
	);
}

export function OutputEditor({
	output,
	busy,
	onSave,
	onCancel,
	mode = "all",
}: {
	output: OutputConfigurationView;
	busy: boolean;
	onSave: (edit: UpdateOutputConfiguration) => void;
	onCancel: () => void;
	mode?: "all" | "picture" | "dmx";
}) {
	const [targetKind, setTargetKind] = useState(output.targetKind);
	const [monitorBy, setMonitorBy] = useState(output.monitorBy ?? "index");
	const [monitorValue, setMonitorValue] = useState(output.monitorValue ?? "0");
	const [fullscreen, setFullscreen] = useState(output.fullscreen);
	const [width, setWidth] = useState(output.width);
	const [height, setHeight] = useState(output.height);
	const [presentation, setPresentation] = useState(output.presentation);
	const [framesPerSecond, setFramesPerSecond] = useState(
		output.framesPerSecond ?? 60,
	);
	const [personality, setPersonality] = useState(output.personality);
	const [protocol, setProtocol] = useState(output.protocol);
	const [universe, setUniverse] = useState(output.universe);
	const [startAddress, setStartAddress] = useState(output.startAddress);

	return (
		<form
			className="media-settings-form"
			onSubmit={(event) => {
				event.preventDefault();
				onSave({
					requestId: requestId(),
					targetKind,
					...(targetKind === "monitor"
						? { monitorBy, monitorValue: monitorValue.trim(), fullscreen }
						: {}),
					width,
					height,
					presentation,
					...(presentation === "fixed-fps" ? { framesPerSecond } : {}),
					personality,
					protocol,
					universe,
					startAddress,
				});
			}}
		>
			{mode !== "dmx" && (
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
								label="Find monitor by"
								value={monitorBy}
								options={[
									{ value: "index", label: "Monitor number" },
									{ value: "name", label: "Monitor name" },
								]}
								onChange={setMonitorBy}
							/>
							{monitorBy === "index" ? (
								<NumberField
									label="Monitor number"
									description="The zero-based number reported by this machine. An unavailable monitor is an error, never a fallback to another display."
									min={0}
									step={1}
									value={monitorValue}
									onChange={(event) => setMonitorValue(event.target.value)}
								/>
							) : (
								<TextField
									label="Monitor name"
									description="Exactly as this machine reports it. An unavailable monitor is an error, never a fallback to another display."
									value={monitorValue}
									onChange={(event) => setMonitorValue(event.target.value)}
								/>
							)}
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

			{mode !== "dmx" && (
				<PictureFields
					{...{
						width,
						height,
						presentation,
						framesPerSecond,
						setWidth,
						setHeight,
						setPresentation,
						setFramesPerSecond,
					}}
				/>
			)}

			{mode !== "picture" && (
				<DmxInputFields
					{...{
						personality,
						protocol,
						universe,
						startAddress,
						setPersonality,
						setProtocol,
						setUniverse,
						setStartAddress,
					}}
				/>
			)}

			{output.takesEffectOnRestart && <RestartNotice />}

			<div className="media-settings-actions">
				<Button type="submit" variant="primary" loading={busy}>
					{mode === "dmx" ? "Save DMX input" : "Save output settings"}
				</Button>
				<Button onClick={onCancel}>Cancel</Button>
			</div>
		</form>
	);
}
