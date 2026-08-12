import {
	Button,
	ColorPickerField,
	FormLayout,
	NumberField,
	SelectField,
	TextAreaField,
	TextField,
} from "@tosklight/ui/controls";
import { DataTable, WindowFrame } from "@tosklight/ui/window-kit";
import { type ReactNode, useState } from "react";
import {
	MediaListDetail,
	MediaMetric,
	MediaPanel,
	MediaPreview,
	MediaScreenHeader,
	MediaSettingsLayout,
	type MediaSettingsSection,
} from "./MediaServerSurface";

export interface OperatorOutput {
	id: string;
	name: string;
	target: string;
	resolution: string;
	status: string;
	layers: Array<{ id: string; name: string; source: string; level: string }>;
}

export interface OperatorLibraryItem {
	id: string;
	name: string;
	address: string;
	type: string;
	thumbnail?: string;
}

export interface OperatorVisualizer {
	id: string;
	name: string;
	address: string;
	kind: string;
	controls: string[];
	variant?: "aurora" | "particles";
}

export interface OperatorTextSource {
	id: string;
	name: string;
	address: string;
	kind: string;
	text: string;
	enabled: boolean;
}

function StorySelect<T extends string>({
	label,
	initialValue,
	options,
}: {
	label: string;
	initialValue: T;
	options: Array<{ value: T; label: string }>;
}) {
	const [value, setValue] = useState(initialValue);
	return (
		<SelectField
			label={label}
			value={value}
			options={options}
			onChange={setValue}
		/>
	);
}

export function DashboardScreen({
	instance,
	showName,
	outputs,
	libraryItems,
	dmxRate,
	recent,
}: {
	instance: string;
	showName?: string;
	outputs: OperatorOutput[];
	libraryItems: number;
	dmxRate: string;
	recent: ReactNode;
}) {
	return (
		<WindowFrame
			title="Dashboard"
			info={{
				primary: instance,
				secondary: showName
					? `Connected to ${showName}. The server is ready for this show.`
					: "This Media Server is ready. No Light Desk is currently identified.",
			}}
			className="media-dashboard-window"
		>
			<div className="media-dashboard-content">
				<div className="media-metric-grid">
					<MediaMetric
						label="Light Desk"
						value={showName ?? "Not connected"}
						detail={showName ? "Active show" : "Waiting for identity"}
						tone={showName ? "good" : "warn"}
					/>
					<MediaMetric
						label="Outputs"
						value={String(outputs.length)}
						detail={`${outputs.filter((output) => output.status === "Live").length} live`}
						tone="good"
					/>
					<MediaMetric
						label="Library"
						value={String(libraryItems)}
						detail="Addressable items"
					/>
					<MediaMetric
						label="DMX input"
						value={dmxRate}
						detail="Accepted update rate"
						tone="good"
					/>
				</div>
				<div className="media-operator-card-grid">
					<MediaPanel
						title="Outputs"
						detail="Current picture surfaces and render health."
					>
						<DataTable
							rows={outputs}
							rowKey={(output) => output.id}
							columns={[
								{
									id: "name",
									header: "Output",
									render: (output) => output.name,
								},
								{
									id: "target",
									header: "Target",
									render: (output) => output.target,
								},
								{
									id: "picture",
									header: "Picture",
									render: (output) => output.resolution,
								},
								{
									id: "status",
									header: "Status",
									render: (output) => output.status,
								},
							]}
						/>
					</MediaPanel>
					<MediaPanel
						title="Recent activity"
						detail="The events an operator is most likely to act on."
					>
						<div className="media-recent-activity">{recent}</div>
					</MediaPanel>
				</div>
			</div>
		</WindowFrame>
	);
}

export function MediaScreen({ outputs }: { outputs: OperatorOutput[] }) {
	return (
		<>
			<MediaScreenHeader
				title="Media"
				detail="See the source and level currently resolved on every output layer."
			/>
			<div className="media-operator-card-grid">
				{outputs.map((output) => (
					<MediaPanel
						key={output.id}
						title={output.name}
						detail={`${output.target} · ${output.resolution}`}
					>
						<MediaPreview title={`${output.name} program`} variant="media" />
						<DataTable
							rows={output.layers}
							rowKey={(layer) => layer.id}
							columns={[
								{ id: "name", header: "Layer", render: (layer) => layer.name },
								{
									id: "source",
									header: "Source",
									render: (layer) => layer.source,
								},
								{
									id: "level",
									header: "Level",
									render: (layer) => layer.level,
								},
							]}
						/>
					</MediaPanel>
				))}
			</div>
		</>
	);
}

export function LibraryScreen({
	items,
	selectedId,
	onSelect,
}: {
	items: OperatorLibraryItem[];
	selectedId: string;
	onSelect?: (id: string) => void;
}) {
	const selected = items.find((item) => item.id === selectedId) ?? items[0];
	return (
		<>
			<MediaScreenHeader
				title="Library"
				detail="Prepare stable, numbered media without changing the address a show uses."
				actions={
					<div className="media-operator-toolbar">
						<Button variant="primary">Import media</Button>
					</div>
				}
			/>
			<MediaListDetail
				label="Library media"
				items={items.map((item) => ({
					id: item.id,
					title: item.name,
					detail: `${item.type} · ${item.address}`,
					meta: "Ready",
				}))}
				selectedId={selected?.id ?? ""}
				onSelect={onSelect}
				detail={
					selected ? (
						<>
							<MediaPreview title={selected.name} variant="media" />
							<h2>{selected.name}</h2>
							<p>
								{selected.address} · {selected.type}
							</p>
							<div className="media-operator-toolbar">
								<Button>Rename</Button>
								<Button>Move</Button>
								<Button>Replace source</Button>
							</div>
						</>
					) : (
						<p>No media is selected.</p>
					)
				}
			/>
		</>
	);
}

export function VisualizersScreen({
	items,
	selectedId,
	onSelect,
}: {
	items: OperatorVisualizer[];
	selectedId: string;
	onSelect?: (id: string) => void;
}) {
	const selected = items.find((item) => item.id === selectedId) ?? items[0];
	return (
		<>
			<MediaScreenHeader
				title="Visualizers"
				detail="Select a generated source to inspect its live look and tune its published controls."
			/>
			<MediaListDetail
				label="Visualizers"
				items={items.map((item) => ({
					id: item.id,
					title: item.name,
					detail: item.kind,
					meta: item.address,
				}))}
				selectedId={selected?.id ?? ""}
				onSelect={onSelect}
				detail={
					selected ? (
						<>
							<MediaPreview title={selected.name} variant={selected.variant} />
							<h2>{selected.name}</h2>
							<p>
								{selected.kind} · {selected.address}
							</p>
							<FormLayout columns={2} className="media-operator-control-row">
								{selected.controls.slice(0, 4).map((control, index) => (
									<NumberField
										key={control}
										label={control}
										min={0}
										max={100}
										defaultValue={72 - index * 9}
									/>
								))}
							</FormLayout>
							<div className="media-operator-toolbar">
								<Button variant="primary">Save visualizer</Button>
							</div>
						</>
					) : (
						<p>No visualizer is selected.</p>
					)
				}
			/>
		</>
	);
}

export function TextScreen({
	items,
	selectedId,
	onSelect,
	onTextChange,
}: {
	items: OperatorTextSource[];
	selectedId: string;
	onSelect?: (id: string) => void;
	onTextChange?: (text: string) => void;
}) {
	const selected = items.find((item) => item.id === selectedId) ?? items[0];
	return (
		<>
			<MediaScreenHeader
				title="Text"
				detail="Write and style addressable text, clocks, and countdowns."
				actions={
					<div className="media-operator-toolbar">
						<Button variant="primary">New text source</Button>
					</div>
				}
			/>
			<MediaListDetail
				label="Text sources"
				items={items.map((item) => ({
					id: item.id,
					title: item.name,
					detail: item.kind,
					meta: item.address,
				}))}
				selectedId={selected?.id ?? ""}
				onSelect={onSelect}
				detail={
					selected ? (
						<>
							<MediaPreview title={selected.name} variant="text">
								<span className="media-text-preview-words">
									{selected.text}
								</span>
							</MediaPreview>
							<FormLayout columns={2} className="media-operator-form-grid">
								<TextAreaField
									label="Text"
									value={selected.text}
									onChange={(event) => onTextChange?.(event.target.value)}
								/>
								<div className="media-operator-control-row">
									<StorySelect
										label="Font"
										initialValue="sans"
										options={[
											{ value: "sans", label: "Sans serif" },
											{ value: "serif", label: "Serif" },
										]}
									/>
									<StorySelect
										label="Alignment"
										initialValue="center"
										options={[
											{ value: "left", label: "Left" },
											{ value: "center", label: "Centre" },
											{ value: "right", label: "Right" },
										]}
									/>
									<NumberField label="Height" defaultValue={20} />
									<ColorPickerField
										label="Colour"
										value="#ffffff"
										onChange={() => {}}
									/>
								</div>
							</FormLayout>
							<div className="media-operator-toolbar">
								<Button variant="primary">Save text</Button>
								<Button variant="danger">Remove</Button>
							</div>
						</>
					) : (
						<p>No text source is selected.</p>
					)
				}
			/>
		</>
	);
}

export function SettingsScreen({
	active,
	onSelect,
	children,
}: {
	active: MediaSettingsSection;
	onSelect?: (section: MediaSettingsSection) => void;
	children: ReactNode;
}) {
	return (
		<MediaSettingsLayout active={active} onSelect={onSelect}>
			{children}
		</MediaSettingsLayout>
	);
}

function SettingsSection({
	title,
	detail,
	className,
	children,
}: {
	title: string;
	detail: string;
	className?: string;
	children: ReactNode;
}) {
	return (
		<section
			className={["media-settings-section", className]
				.filter(Boolean)
				.join(" ")}
		>
			<div className="media-settings-section-heading">
				<h3>{title}</h3>
				<p>{detail}</p>
			</div>
			{children}
		</section>
	);
}

export function LibrariesSettings({
	root = "/Users/Shared/ToskLight/Media",
}: {
	root?: string;
}) {
	return (
		<>
			<h2>Libraries</h2>
			<p>
				Choose where authored sources, playable media, thumbnails, and generated
				sources are kept.
			</p>
			<FormLayout columns={2} className="media-operator-control-row">
				<TextField label="Library folder" value={root} readOnly />
				<StorySelect
					label="Playback format"
					initialValue="hap-alpha"
					options={[{ value: "hap-alpha", label: "HAP Alpha" }]}
				/>
			</FormLayout>
			<div className="media-operator-toolbar">
				<Button>Choose folder</Button>
				<Button variant="primary">Save library settings</Button>
			</div>
		</>
	);
}

export function OutputsSettings() {
	return (
		<>
			<h2>Outputs</h2>
			<p>Choose the picture surface and presentation for each output.</p>
			<SettingsSection title="Main output" detail="Picture surface 1">
				<FormLayout columns={2} className="media-operator-control-row">
					<StorySelect
						label="Output target"
						initialValue="display-2"
						options={[
							{ value: "display-2", label: "Display 2 · 1920 × 1080" },
							{ value: "offscreen", label: "Off-screen surface" },
						]}
					/>
					<StorySelect
						label="Picture size"
						initialValue="1920"
						options={[
							{ value: "1920", label: "1920 × 1080" },
							{ value: "1280", label: "1280 × 720" },
						]}
					/>
					<StorySelect
						label="Presentation"
						initialValue="display"
						options={[
							{ value: "display", label: "Display synchronized" },
							{ value: "fixed", label: "Fixed rate" },
							{ value: "diagnostic", label: "Diagnostic pattern" },
						]}
					/>
					<StorySelect
						label="Orientation"
						initialValue="normal"
						options={[
							{ value: "normal", label: "Landscape" },
							{ value: "left", label: "Portrait left" },
						]}
					/>
					<StorySelect
						label="Sound output"
						initialValue="display-2"
						options={[
							{ value: "disabled", label: "Muted" },
							{ value: "system", label: "System default" },
							{ value: "display-2", label: "Display 2 audio" },
						]}
					/>
				</FormLayout>
				<div className="media-operator-toolbar">
					<Button variant="primary">Save output</Button>
				</div>
			</SettingsSection>
		</>
	);
}

export function NetworkInputsSettings() {
	return (
		<>
			<h2>Network &amp; Inputs</h2>
			<p>Keep network listeners, DMX identity, and audio input together.</p>
			<div className="media-operator-card-grid">
				<SettingsSection title="Network" detail="Addresses used after restart">
					<FormLayout columns={2} className="media-operator-control-row">
						<TextField label="Art-Net listen" defaultValue="0.0.0.0:6454" />
						<TextField label="sACN listen" defaultValue="0.0.0.0:5568" />
						<TextField label="CITP listen" defaultValue="0.0.0.0:4809" />
						<TextField label="This interface" defaultValue="0.0.0.0:4711" />
					</FormLayout>
				</SettingsSection>
				<SettingsSection
					title="DMX"
					detail="The block patched on the Light Desk"
				>
					<FormLayout columns={2} className="media-operator-control-row">
						<StorySelect
							label="Personality"
							initialValue="8"
							options={[
								{ value: "2", label: "2 layers" },
								{ value: "8", label: "8 layers" },
							]}
						/>
						<StorySelect
							label="Protocol"
							initialValue="artnet"
							options={[
								{ value: "artnet", label: "Art-Net" },
								{ value: "sacn", label: "sACN" },
							]}
						/>
						<NumberField label="Universe" defaultValue={1} />
						<NumberField label="Start address" defaultValue={1} />
					</FormLayout>
				</SettingsSection>
			</div>
			<SettingsSection
				title="Audio input"
				detail="Feeds audio-reactive visualizers"
				className="media-audio-input-panel"
			>
				<FormLayout columns={2} className="media-operator-control-row">
					<StorySelect
						label="Input device"
						initialValue="usb"
						options={[
							{ value: "usb", label: "USB Audio CODEC" },
							{ value: "system", label: "System default" },
						]}
					/>
					<NumberField label="Gain" min={0} max={100} defaultValue={72} />
				</FormLayout>
			</SettingsSection>
			<div className="media-operator-toolbar">
				<Button variant="primary">Save network &amp; inputs</Button>
			</div>
		</>
	);
}

export function LogsSettings() {
	return (
		<>
			<h2>Logs</h2>
			<p>
				Filter the visible log separately from the level captured by the running
				server.
			</p>
			<FormLayout columns={2} className="media-operator-control-row">
				<StorySelect
					label="Show"
					initialValue="info"
					options={[
						{ value: "debug", label: "Debug and above" },
						{ value: "info", label: "Info and above" },
						{ value: "warn", label: "Warnings and errors" },
					]}
				/>
				<StorySelect
					label="Server log level"
					initialValue="info"
					options={[
						{ value: "debug", label: "Debug" },
						{ value: "info", label: "Info" },
						{ value: "warn", label: "Warn" },
					]}
				/>
			</FormLayout>
			<MediaPanel className="media-operator-log" title="Server log">
				<pre>
					23:31:09 INFO Output Main presented frame 18420{"\n"}23:31:08 INFO
					Light Desk identified show “The Tempest”{"\n"}23:31:07 INFO Art-Net
					input active at 44 Hz
				</pre>
			</MediaPanel>
		</>
	);
}
