import type {
	PatchFixtureProjection,
	PatchProfileRevision,
} from "@tosklight/patch";
import { Button, type TitleActionGroup } from "@tosklight/ui";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import {
	type CSSProperties,
	type ReactNode,
	type RefObject,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import {
	DMX_SLOTS,
	type DmxOccupancy,
	type DmxOccupant,
	dmxChannelsPerRow,
	dmxOccupancy,
	fixtureTitle,
	patchedUniverses,
} from "./dmxChannels";
import {
	type DocumentSummary,
	documentSession,
	type LiveDmxProtocol,
	type NetworkNode,
	type NetworkNodePort,
	type NetworkSentUniverse,
	type NetworkSources,
	type ReceivedDmx,
	type ReceivingInput,
} from "./document/session";
import { DmxInterfacesPanel } from "./DmxInterfacesPanel";
import { LiveDmxInputsPanel } from "./LiveDmxInputsPanel";
import { useDiscoveredDesks } from "./useDiscoveredDesks";
import { beginWindowDrag } from "./WindowChrome";

export type DmxPage = "network" | "values" | "sources";

export interface Channel {
	universe: number;
	address: number;
}

/** A received value is a dot. */
const VALUE_DOT = 9;
const DIP_WEIGHTS = [1, 2, 4, 8, 16, 32, 64, 128, 256];

/**
 * The Architect's DMX settings page.
 *
 * **Network** is where the show's DMX arrives from, and **Values** is what actually arrives — the
 * desk's DMX Output window, reading the network instead of a desk. The Architect outputs nothing, so
 * there is nothing here to override, and Values has no window settings.
 * **Sources** is who is on the network: every Art-Net node and sACN source, and their universes.
 * Which addresses the rig occupies is the Patch screen's DMX tab, not a page here.
 *
 * It is a page of Settings, so its own tabs sit left of the Settings pages in the one title.
 */
export function DmxWorkspace({
	page,
	onPage,
	document,
	fixtures,
	profileRevisions,
	settingsPages,
	onError,
}: {
	/** The Settings page tabs, drawn right of this page's own groups. */
	settingsPages?: TitleActionGroup;
	page: DmxPage;
	onPage: (page: DmxPage) => void;
	document: DocumentSummary;
	fixtures: readonly PatchFixtureProjection[];
	profileRevisions: readonly PatchProfileRevision[];
	onError: (reason: unknown) => void;
}) {
	const desks = useDiscoveredDesks();
	const occupancy = useMemo(
		() => dmxOccupancy(fixtures, profileRevisions),
		[fixtures, profileRevisions],
	);

	return (
		<section className="viz-dmx-workspace">
			<WindowHeader
				title="Settings"
				dragHandleProps={{
					"data-tauri-drag-region": true,
					onPointerDown: beginWindowDrag,
				}}
				groups={[
					{
						id: "dmx-pages",
						kind: "tabs",
						activeId: page,
						onActiveChange: (id) => onPage(id as DmxPage),
						actions: [
							{ id: "network", label: "Network" },
							{ id: "values", label: "Values" },
							{ id: "sources", label: "Sources" },
						],
					},
					...(settingsPages ? [settingsPages] : []),
				]}
			/>
			{page === "network" ? (
				<div className="viz-dmx-network">
					<DmxInterfacesPanel onError={onError} />
					<LiveDmxInputsPanel
						document={document}
						desks={desks}
						onError={onError}
					/>
				</div>
			) : null}
			{page === "sources" ? <DmxSourcesView /> : null}
			{page === "values" ? (
				<DmxValuesView occupancy={occupancy} />
			) : null}
		</section>
	);
}

/** The desk's DMX Output window, reading what arrives over Art-Net and sACN instead. */
function DmxValuesView({ occupancy }: { occupancy: DmxOccupancy }) {
	const received = useReceivedDmx();
	const [selected, setSelected] = useState<Channel | null>(null);
	const host = useRef<HTMLElement>(null);
	const dot = VALUE_DOT;
	const columns = dmxChannelsPerRow(useWidth(host), dot);
	const universes = valueUniverses(received.value, occupancy);
	const selectedUniverse = selected
		? universes.find((frame) => frame.universe === selected.universe)
		: undefined;

	return (
		<div className="viz-dmx-content">
			<WindowScrollArea>
				<main ref={host} style={gridStyle(columns, dot)}>
					{universes.map((frame) => (
						<section
							className="viz-dmx-universe is-values dots-small"
							key={frame.universe}
							aria-label={`Universe ${frame.universe} values`}
						>
							<header>
								<b>
									Universe {frame.universe} · channels 1–{DMX_SLOTS}
								</b>
								<small>
										{universeState(frame, received.value?.inputs ?? [])}
									</small>
							</header>
							{rows(
								columns,
								(address) => {
									const value = frame.slots?.[address - 1] ?? 0;
									const isSelected =
										selected?.universe === frame.universe &&
										selected.address === address;
									return (
										<button
											type="button"
											key={address}
											className={classes({
												high: value > 210,
												mid: value > 90 && value <= 210,
												low: value > 20 && value <= 90,
												"is-patched": Boolean(
													occupancy.get(frame.universe)?.get(address)?.length,
												),
												"is-selected": isSelected,
											})}
											aria-pressed={isSelected}
											aria-label={`Universe ${frame.universe}, address ${address}, value ${value}`}
											onClick={() =>
												setSelected({ universe: frame.universe, address })
											}
										/>
									);
								},
								hexAddress,
							)}
						</section>
					))}
					{received.value && !universes.length ? (
						<p className="viz-dmx-empty">
							Nothing to listen for. Patch a fixture, or add an input on the
							Network tab.
						</p>
					) : null}
					{!received.value && !received.error ? (
						<p className="viz-dmx-empty">Opening the DMX inputs…</p>
					) : null}
				</main>
			</WindowScrollArea>
			<aside className="viz-dmx-info">
				{selected ? (
					<ChannelInfo
						channel={selected}
						occupants={
							occupancy.get(selected.universe)?.get(selected.address) ?? []
						}
						value={
							selectedUniverse?.slots
								? selectedUniverse.slots[selected.address - 1]
								: null
						}
						onDeselect={() => setSelected(null)}
					/>
				) : (
					<ReceiveSummary received={received.value} error={received.error} />
				)}
			</aside>
		</div>
	);
}

/** Poll what arrived while this is on screen, and stop listening when it is not. */
function useReceivedDmx() {
	return usePolledWhileShown(
		documentSession.receivedDmx,
		documentSession.stopReceivedDmx,
		250,
	);
}

/** Look for nodes while the Sources tab is on screen, and stop polling the network when it is not. */
function useNetworkSources() {
	return usePolledWhileShown(
		documentSession.networkSources,
		documentSession.stopNetworkSources,
		1000,
	);
}

function usePolledWhileShown<T>(
	load: () => Promise<T>,
	stop: () => Promise<void>,
	intervalMillis: number,
) {
	const [value, setValue] = useState<T | null>(null);
	const [error, setError] = useState("");
	useEffect(() => {
		let active = true;
		let timer: number | undefined;
		const read = () => {
			load()
				.then((next) => {
					if (!active) return;
					setValue(next);
					setError("");
				})
				.catch((reason) => {
					if (active) setError(String(reason));
				})
				.finally(() => {
					// One read at a time: the next is scheduled only once this one answered.
					if (active) timer = window.setTimeout(read, intervalMillis);
				});
		};
		read();
		return () => {
			active = false;
			window.clearTimeout(timer);
			void stop().catch(() => undefined);
		};
	}, [load, stop, intervalMillis]);
	return { value, error };
}

/** Every Art-Net node and sACN source this computer finds, and the universes each one carries. */
function DmxSourcesView() {
	const sources = useNetworkSources();
	const [selected, setSelected] = useState<string | null>(null);
	const nodes = sources.value?.nodes ?? [];
	const node = nodes.find((candidate) => candidate.address === selected);

	return (
		<div className="viz-dmx-content">
			<WindowScrollArea>
				<main>
					{nodes.length ? (
						<div className="viz-dmx-sources-wrap">
							<table className="viz-dmx-sources" aria-label="Network sources">
								<thead>
									<tr>
										<th>Node</th>
										<th>IP address</th>
										<th>Protocols</th>
										<th>Inputs</th>
										<th>Outputs</th>
										<th>Sends</th>
										<th>Receives</th>
									</tr>
								</thead>
								<tbody>
									{nodes.map((candidate) => {
										const isSelected = candidate.address === selected;
										return (
											<tr
												key={candidate.address}
												className={isSelected ? "is-selected" : undefined}
											>
												<td>
													<button
														type="button"
														aria-pressed={isSelected}
														onClick={() =>
															setSelected(isSelected ? null : candidate.address)
														}
													>
														{candidate.name || "Unnamed node"}
													</button>
													{candidate.longName &&
													candidate.longName !== candidate.name ? (
														<small>{candidate.longName}</small>
													) : null}
												</td>
												<td>
													<code>{candidate.address}</code>
												</td>
												<td>{candidate.protocols.map(protocolLabel).join(", ")}</td>
												<td>
													<PortList ports={candidate.inputs} direction="input" />
												</td>
												<td>
													<PortList ports={candidate.outputs} direction="output" />
												</td>
												<td>
													<UniverseChips universes={candidate.sends} />
												</td>
												<td>
													<UniverseChips universes={receivedUniverses(candidate)} />
												</td>
											</tr>
										);
									})}
								</tbody>
							</table>
						</div>
					) : (
						<p className="viz-dmx-empty">
							{sources.value
								? "No Art-Net node or sACN source found yet. Art-Net nodes answer within a few seconds; sACN sources announce every ten seconds."
								: "Looking for Art-Net nodes and sACN sources…"}
						</p>
					)}
				</main>
			</WindowScrollArea>
			<aside className="viz-dmx-info">
				{node ? (
					<NodeInfo node={node} onDeselect={() => setSelected(null)} />
				) : (
					<SourcesSummary sources={sources.value} error={sources.error} />
				)}
			</aside>
		</div>
	);
}

/** What a node plays out of its DMX outputs is what it receives from the network. */
function receivedUniverses(node: NetworkNode): NetworkSentUniverse[] {
	const universes = new Map<number, NetworkSentUniverse>();
	for (const port of node.outputs) {
		const known = universes.get(port.universe);
		universes.set(port.universe, {
			protocol: "artnet",
			universe: port.universe,
			announced: true,
			live: port.active || Boolean(known?.live),
		});
	}
	return [...universes.values()].sort((left, right) => left.universe - right.universe);
}

function PortList({
	ports,
	direction,
}: {
	ports: readonly NetworkNodePort[];
	direction: "input" | "output";
}) {
	if (!ports.length) return <span className="viz-dmx-none">—</span>;
	return (
		<ul className="viz-dmx-ports">
			{ports.map((port) => (
				<li key={port.label} className={port.active ? "is-live" : undefined}>
					{direction === "input"
						? `${port.label} → Universe ${port.universe}`
						: `Universe ${port.universe} → ${port.label}`}
					{port.kind === "DMX512" ? "" : ` · ${port.kind}`}
				</li>
			))}
		</ul>
	);
}

function UniverseChips({
	universes,
}: {
	universes: readonly NetworkSentUniverse[];
}) {
	if (!universes.length) return <span className="viz-dmx-none">—</span>;
	return (
		<ul className="viz-dmx-chips">
			{universes.map((universe) => (
				<li
					key={`${universe.protocol}-${universe.universe}`}
					className={universe.live ? "is-live" : undefined}
					title={`${universe.live ? "Data arriving now" : "No data arriving now"} · ${
						universe.announced ? "reported by the node" : "seen on the network"
					}`}
				>
					{protocolLabel(universe.protocol)} {universe.universe}
				</li>
			))}
		</ul>
	);
}

function NodeInfo({
	node,
	onDeselect,
}: {
	node: NetworkNode;
	onDeselect: () => void;
}) {
	return (
		<>
			<header className="viz-dmx-info-header">
				<b>Selected node</b>
				<Button size="compact" onClick={onDeselect}>
					Deselect
				</Button>
			</header>
			<section className="viz-dmx-fixture-card">
				<b>Node</b>
				<dl>
					<dt>Name</dt>
					<dd>{node.name || "—"}</dd>
					<dt>Long name</dt>
					<dd>{node.longName || "—"}</dd>
					<dt>IP address</dt>
					<dd>{node.address}</dd>
					<dt>MAC address</dt>
					<dd>{node.mac ?? "—"}</dd>
					<dt>Protocols</dt>
					<dd>{node.protocols.map(protocolLabel).join(", ")}</dd>
					<dt>Last heard</dt>
					<dd>{(node.lastSeenMillis / 1000).toFixed(1)} s ago</dd>
				</dl>
			</section>
			{node.report ? (
				<section>
					<b>Status report</b>
					<p>{node.report}</p>
				</section>
			) : null}
		</>
	);
}

function SourcesSummary({
	sources,
	error,
}: {
	sources: NetworkSources | null;
	error: string;
}) {
	const nodes = sources?.nodes ?? [];
	const artNet = nodes.filter((node) => node.protocols.includes("artnet")).length;
	const sacn = nodes.filter((node) => node.protocols.includes("sacn")).length;
	return (
		<>
			<b>Network summary</b>
			{error ? (
				<output className="viz-dmx-error" role="alert">
					{error}
				</output>
			) : null}
			<section>
				<b>Found</b>
				<p>
					{count(artNet, "Art-Net node")} · {count(sacn, "sACN source")}
				</p>
			</section>
			<section>
				<b>Polling</b>
				{sources?.polling.length ? (
					<ul className="viz-dmx-list">
						{sources.polling.map((address) => (
							<li key={address}>
								<span>Art-Net poll to {address}</span>
							</li>
						))}
					</ul>
				) : (
					<p>No network to poll</p>
				)}
			</section>
			{sources?.warnings.length ? (
				<section>
					<b>Notes</b>
					{sources.warnings.map((warning) => (
						<p key={warning}>{warning}</p>
					))}
				</section>
			) : null}
			<section>
				<b>sACN</b>
				<p>
					sACN receivers do not announce themselves, so only sources are listed: with the
					universes they announce, and the show's universes they are seen sending.
				</p>
			</section>
		</>
	);
}

function count(amount: number, noun: string) {
	return `${amount} ${noun}${amount === 1 ? "" : "s"}`;
}

export function ChannelInfo({
	channel,
	occupants,
	value,
	onDeselect,
}: {
	channel: Channel;
	occupants: readonly DmxOccupant[];
	/** The received value; `null` when nothing has arrived, absent on the Patch tab. */
	value?: number | null;
	onDeselect: () => void;
}) {
	const { universe, address } = channel;
	return (
		<>
			<header className="viz-dmx-info-header">
				<b>Selected channel</b>
				<Button size="compact" onClick={onDeselect}>
					Deselect
				</Button>
			</header>
			<section className="viz-dmx-address-card">
				<strong>
					Universe {universe} · Channel {address}
				</strong>
				<small>
					DMX address {address} · {hexAddress(address)}
				</small>
				<div
					className="viz-dmx-dip-switches"
					aria-label={`DIP switches for DMX address ${address}`}
				>
					{DIP_WEIGHTS.map((weight) => (
						<span className={address & weight ? "on" : ""} key={weight}>
							<i aria-hidden="true" />
							<small>{weight}</small>
						</span>
					))}
				</div>
			</section>
			{value !== undefined ? (
				<section className="viz-dmx-value-card">
					<b>Received value</b>
					{value === null ? (
						<p>Nothing received</p>
					) : (
						<>
							<strong>{value}</strong>
							<small>
								{Math.round((value / 255) * 100)}% · 0x
								{value.toString(16).toUpperCase().padStart(2, "0")}
							</small>
						</>
					)}
				</section>
			) : null}
			<section className="viz-dmx-fixture-card">
				<b>Fixture</b>
				{occupants.length ? (
					occupants.map((occupant) => (
						<dl key={`${occupant.fixture.fixtureId}-${occupant.owner}`}>
							<dt>Fixture</dt>
							<dd>{fixtureTitle(occupant.fixture)}</dd>
							<dt>Patch owner</dt>
							<dd>{occupant.owner}</dd>
							<dt>Patch range</dt>
							<dd>
								{universe}.{occupant.start}–
								{occupant.start + occupant.footprint - 1}
							</dd>
							<dt>Split</dt>
							<dd>{occupant.split}</dd>
							<dt>Fixture channel</dt>
							<dd>
								{occupant.channel} of {occupant.footprint}
							</dd>
							<dt>Attribute</dt>
							<dd>{occupant.attribute ?? "—"}</dd>
						</dl>
					))
				) : (
					<p>Not patched</p>
				)}
				{occupants.length > 1 ? (
					<p className="viz-dmx-conflict">
						{occupants.length} patches share this address.
					</p>
				) : null}
			</section>
		</>
	);
}

export function PatchSummary({ occupancy }: { occupancy: DmxOccupancy }) {
	const universes = patchedUniverses(occupancy);
	const conflicts = universes.reduce(
		(count, universe) =>
			count +
			[...(occupancy.get(universe)?.values() ?? [])].filter(
				(occupants) => occupants.length > 1,
			).length,
		0,
	);
	return (
		<>
			<b>Patch summary</b>
			<section>
				<b>Universes</b>
				{universes.length ? (
					<ul className="viz-dmx-list">
						{universes.map((universe) => (
							<li key={universe}>
								<span>Universe {universe}</span>
								<small>{occupancy.get(universe)?.size ?? 0} patched</small>
							</li>
						))}
					</ul>
				) : (
					<p>Nothing patched</p>
				)}
			</section>
			<section>
				<b>Overlaps</b>
				<p>
					{conflicts
						? `${conflicts} address${conflicts === 1 ? "" : "es"} carry more than one patch`
						: "None"}
				</p>
			</section>
		</>
	);
}

function ReceiveSummary({
	received,
	error,
}: {
	received: ReceivedDmx | null;
	error: string;
}) {
	return (
		<>
			<b>Input summary</b>
			{error ? (
				<output className="viz-dmx-error" role="alert">
					{error}
				</output>
			) : null}
			<section>
				<b>Listening</b>
				{received?.inputs.length ? (
					<ul className="viz-dmx-list">
						{received.inputs.map((input) => (
							<li key={input.id}>
								<span>
									{protocolLabel(input.protocol)} {input.destinationUniverse} →
									Universe {input.logicalUniverse}
								</span>
								<small>
									{input.health}
									{input.source ? ` · ${input.source}` : ""} ·{" "}
									{input.acceptedPackets} packets
								</small>
								{input.detail ? <small>{input.detail}</small> : null}
							</li>
						))}
					</ul>
				) : (
					<p>No inputs</p>
				)}
			</section>
			{received?.warnings.length ? (
				<section>
					<b>Notes</b>
					{received.warnings.map((warning) => (
						<p key={warning}>{warning}</p>
					))}
				</section>
			) : null}
		</>
	);
}

/**
 * Every received universe and every patched one, in order. A patched universe nothing arrives on
 * still shows all its channels, so the operator sees what the rig expects there.
 */
function valueUniverses(
	received: ReceivedDmx | null,
	occupancy: DmxOccupancy,
): ReceivedDmx["universes"] {
	const frames = new Map(
		(received?.universes ?? []).map((frame) => [frame.universe, frame]),
	);
	for (const universe of patchedUniverses(occupancy))
		if (!frames.has(universe))
			frames.set(universe, {
				universe,
				slots: null,
				live: false,
				rateHz: 0,
				protocol: null,
			});
	return [...frames.values()].sort((left, right) => left.universe - right.universe);
}

/** Two neighbouring addresses belong to one fixture: each has that fixture as its only occupant. */
export function sameFixture(
	here: readonly DmxOccupant[],
	there: readonly DmxOccupant[] | undefined,
) {
	return (
		here.length === 1 &&
		there?.length === 1 &&
		here[0].fixture.fixtureId === there[0].fixture.fixtureId
	);
}

function universeState(
	frame: ReceivedDmx["universes"][number],
	inputs: readonly ReceivingInput[],
): ReactNode {
	if (frame.live) {
		// The input on this universe that heard the frames names who sends them.
		const source = inputs.find(
			(input) =>
				input.logicalUniverse === frame.universe &&
				input.protocol === frame.protocol &&
				input.source,
		)?.source;
		return `${frame.protocol ? protocolLabel(frame.protocol) : "Live"} · ${frame.rateHz.toFixed(1)} Hz${source ? ` · from ${source}` : ""}`;
	}
	return frame.slots ? "Holding the last frame — source stopped" : "Waiting for DMX";
}

function protocolLabel(protocol: LiveDmxProtocol) {
	return protocol === "sacn" ? "sACN" : "Art-Net";
}

export function occupantLabel(occupant: DmxOccupant) {
	const attribute = occupant.attribute ? ` ${occupant.attribute}` : "";
	return `${fixtureTitle(occupant.fixture)} channel ${occupant.channel}${attribute}`;
}

function hexAddress(address: number) {
	return `0x${address.toString(16).toUpperCase().padStart(3, "0")}`;
}

/** Rows of `columns` channels, each labelled with its first address. */
export function rows(
	columns: number,
	cell: (address: number) => ReactNode,
	label: (address: number) => ReactNode = String,
) {
	return Array.from({ length: Math.ceil(DMX_SLOTS / columns) }, (_, row) => {
		const first = row * columns + 1;
		return (
			<div className="viz-dmx-row" key={first}>
				<code>{label(first)}</code>
				<div>
					{Array.from(
						{ length: Math.min(columns, DMX_SLOTS - first + 1) },
						(_, column) => cell(first + column),
					)}
				</div>
			</div>
		);
	});
}

export function gridStyle(columns: number, cell: number) {
	return {
		"--viz-dmx-columns": columns,
		"--viz-dmx-cell": `${cell}px`,
	} as CSSProperties;
}

export function classes(flags: Record<string, boolean>) {
	return Object.entries(flags)
		.filter(([, on]) => on)
		.map(([name]) => name)
		.join(" ");
}

export function useWidth(host: RefObject<HTMLElement | null>) {
	const [width, setWidth] = useState(900);
	useEffect(() => {
		const node = host.current;
		if (!node || typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(([entry]) => {
			if (entry && entry.contentRect.width > 0)
				setWidth(entry.contentRect.width);
		});
		observer.observe(node);
		return () => observer.disconnect();
	}, [host]);
	return width;
}
