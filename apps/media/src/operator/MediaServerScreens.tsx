import type { ReactNode } from "react";
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
		<>
			<MediaScreenHeader
				eyebrow={instance}
				title="Dashboard"
				detail={
					showName
						? `Connected to ${showName}. The server is ready for this show.`
						: "This Media Server is ready. No Light Desk is currently identified."
				}
			/>
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
					<table className="media-operator-table">
						<thead>
							<tr>
								<th>Output</th>
								<th>Target</th>
								<th>Picture</th>
								<th>Status</th>
							</tr>
						</thead>
						<tbody>
							{outputs.map((output) => (
								<tr key={output.id}>
									<td>{output.name}</td>
									<td>{output.target}</td>
									<td>{output.resolution}</td>
									<td>{output.status}</td>
								</tr>
							))}
						</tbody>
					</table>
				</MediaPanel>
				<MediaPanel
					title="Recent activity"
					detail="The events an operator is most likely to act on."
				>
					{recent}
				</MediaPanel>
			</div>
		</>
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
						<table className="media-operator-table">
							<thead>
								<tr>
									<th>Layer</th>
									<th>Source</th>
									<th>Level</th>
								</tr>
							</thead>
							<tbody>
								{output.layers.map((layer) => (
									<tr key={layer.id}>
										<td>{layer.name}</td>
										<td>{layer.source}</td>
										<td>{layer.level}</td>
									</tr>
								))}
							</tbody>
						</table>
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
						<button type="button" className="is-primary">
							Import media
						</button>
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
								<button type="button">Rename</button>
								<button type="button">Move</button>
								<button type="button">Replace source</button>
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
							<div className="media-operator-control-row">
								{selected.controls.slice(0, 4).map((control, index) => (
									<label key={control}>
										{control}
										<input
											type="range"
											min="0"
											max="100"
											defaultValue={String(72 - index * 9)}
										/>
									</label>
								))}
							</div>
							<div className="media-operator-toolbar">
								<button type="button" className="is-primary">
									Save visualizer
								</button>
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
						<button type="button" className="is-primary">
							New text source
						</button>
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
							<div className="media-operator-form-grid">
								<label className="media-operator-textarea-label">
									Text
									<textarea
										aria-label="Text"
										value={selected.text}
										onChange={(event) => onTextChange?.(event.target.value)}
									/>
								</label>
								<div className="media-operator-control-row">
									<label>
										Font
										<select defaultValue="sans">
											<option value="sans">Sans serif</option>
											<option value="serif">Serif</option>
										</select>
									</label>
									<label>
										Alignment
										<select defaultValue="center">
											<option value="left">Left</option>
											<option value="center">Centre</option>
											<option value="right">Right</option>
										</select>
									</label>
									<label>
										Height
										<input type="number" defaultValue="20" />
									</label>
									<label>
										Colour
										<input type="color" defaultValue="#ffffff" />
									</label>
								</div>
							</div>
							<div className="media-operator-toolbar">
								<button type="button" className="is-primary">
									Save text
								</button>
								<button type="button">Remove</button>
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
		<>
			<MediaScreenHeader
				title="Settings"
				detail="Configure this Media Server with the same focused settings language as the Light Desk."
			/>
			<MediaSettingsLayout active={active} onSelect={onSelect}>
				{children}
			</MediaSettingsLayout>
		</>
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
			<div className="media-operator-control-row">
				<label>
					Library folder
					<input value={root} readOnly />
				</label>
				<label>
					Playback format
					<select defaultValue="hap-alpha">
						<option value="hap-alpha">HAP Alpha</option>
					</select>
				</label>
			</div>
			<div className="media-operator-toolbar">
				<button type="button">Choose folder</button>
				<button type="button" className="is-primary">
					Save library settings
				</button>
			</div>
		</>
	);
}

export function OutputsSettings() {
	return (
		<>
			<h2>Outputs</h2>
			<p>Choose the picture surface and presentation for each output.</p>
			<MediaPanel title="Main output" detail="Picture surface 1">
				<div className="media-operator-control-row">
					<label>
						Output target
						<select defaultValue="display-2">
							<option value="display-2">Display 2 · 1920 × 1080</option>
							<option value="offscreen">Off-screen surface</option>
						</select>
					</label>
					<label>
						Picture size
						<select defaultValue="1920">
							<option value="1920">1920 × 1080</option>
							<option value="1280">1280 × 720</option>
						</select>
					</label>
					<label>
						Presentation
						<select defaultValue="display">
							<option value="display">Display synchronized</option>
							<option value="fixed">Fixed rate</option>
							<option value="diagnostic">Diagnostic pattern</option>
						</select>
					</label>
					<label>
						Orientation
						<select defaultValue="normal">
							<option value="normal">Landscape</option>
							<option value="left">Portrait left</option>
						</select>
					</label>
				</div>
				<div className="media-operator-toolbar">
					<button type="button" className="is-primary">
						Save output
					</button>
				</div>
			</MediaPanel>
		</>
	);
}

export function NetworkInputsSettings() {
	return (
		<>
			<h2>Network &amp; Inputs</h2>
			<p>Keep network listeners, DMX identity, and audio input together.</p>
			<div className="media-operator-card-grid">
				<MediaPanel title="Network" detail="Addresses used after restart">
					<div className="media-operator-control-row">
						<label>
							Art-Net listen
							<input defaultValue="0.0.0.0:6454" />
						</label>
						<label>
							sACN listen
							<input defaultValue="0.0.0.0:5568" />
						</label>
						<label>
							CITP listen
							<input defaultValue="0.0.0.0:4809" />
						</label>
						<label>
							This interface
							<input defaultValue="0.0.0.0:4711" />
						</label>
					</div>
				</MediaPanel>
				<MediaPanel title="DMX" detail="The block patched on the Light Desk">
					<div className="media-operator-control-row">
						<label>
							Personality
							<select defaultValue="8">
								<option value="2">2 layers</option>
								<option value="8">8 layers</option>
							</select>
						</label>
						<label>
							Protocol
							<select defaultValue="artnet">
								<option value="artnet">Art-Net</option>
								<option value="sacn">sACN</option>
							</select>
						</label>
						<label>
							Universe
							<input type="number" defaultValue="1" />
						</label>
						<label>
							Start address
							<input type="number" defaultValue="1" />
						</label>
					</div>
				</MediaPanel>
			</div>
			<MediaPanel title="Audio input" detail="Feeds audio-reactive visualizers">
				<div className="media-operator-control-row">
					<label>
						Input device
						<select defaultValue="usb">
							<option value="usb">USB Audio CODEC</option>
							<option value="system">System default</option>
						</select>
					</label>
					<label>
						Gain
						<input type="range" defaultValue="72" />
					</label>
				</div>
			</MediaPanel>
			<div className="media-operator-toolbar">
				<button type="button" className="is-primary">
					Save network &amp; inputs
				</button>
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
			<div className="media-operator-control-row">
				<label>
					Show
					<select defaultValue="info">
						<option value="debug">Debug and above</option>
						<option value="info">Info and above</option>
						<option value="warn">Warnings and errors</option>
					</select>
				</label>
				<label>
					Server log level
					<select defaultValue="info">
						<option value="debug">Debug</option>
						<option value="info">Info</option>
						<option value="warn">Warn</option>
					</select>
				</label>
			</div>
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
