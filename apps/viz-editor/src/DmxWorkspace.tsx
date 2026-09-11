import type {
	PatchFixtureProjection,
	PatchProfileRevision,
} from "@tosklight/patch";
import { Button } from "@tosklight/ui";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
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
	type ReceivedDmx,
} from "./document/session";
import { DmxInterfacesPanel } from "./DmxInterfacesPanel";
import { LiveDmxInputsPanel } from "./LiveDmxInputsPanel";
import { useDiscoveredDesks } from "./useDiscoveredDesks";
import { beginWindowDrag } from "./WindowChrome";

export type DmxPage = "network" | "patch" | "values";

type DotSize = "small" | "large";
interface Channel {
	universe: number;
	address: number;
}

/** A patch cell carries its address, so it is sized to read one. */
const PATCH_CELL = 34;
const DOT_SIZE_KEY = "tosklight.architect.dmx-dot-size";
const DIP_WEIGHTS = [1, 2, 4, 8, 16, 32, 64, 128, 256];

/**
 * The Architect's DMX screen.
 *
 * **Network** is where the show's DMX arrives from, **Patch** is which addresses the rig occupies,
 * and **Values** is what actually arrives — the desk's DMX Output window, reading the network
 * instead of a desk. The Architect outputs nothing, so there is nothing here to override.
 */
export function DmxWorkspace({
	page,
	onPage,
	document,
	fixtures,
	profileRevisions,
	onError,
}: {
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
	const [dotSize, setDotSize] = useDotSize();
	const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);

	return (
		<section className="viz-dmx-workspace">
			<WindowHeader
				title="DMX"
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
							{ id: "patch", label: "Patch" },
							{ id: "values", label: "Values" },
						],
					},
				]}
				settings={page === "values"}
				onSettings={(anchor) =>
					setSettingsAnchor(anchor.getBoundingClientRect())
				}
			/>
			{settingsAnchor && page === "values" ? (
				<WindowSettings
					modal={false}
					anchor={settingsAnchor}
					title="DMX Settings"
					onClose={() => setSettingsAnchor(null)}
					tabs={[
						{
							id: "display",
							label: "Display",
							content: (
								<>
									<h3>DMX dot size</h3>
									<div className="button-group">
										<Button
											active={dotSize === "small"}
											onClick={() => setDotSize("small")}
										>
											Small
										</Button>
										<Button
											active={dotSize === "large"}
											onClick={() => setDotSize("large")}
										>
											Large
										</Button>
									</div>
								</>
							),
						},
					]}
				/>
			) : null}
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
			{page === "patch" ? <DmxPatchView occupancy={occupancy} /> : null}
			{page === "values" ? (
				<DmxValuesView occupancy={occupancy} dotSize={dotSize} />
			) : null}
		</section>
	);
}

/** Every channel of every patched universe: lit where the rig occupies it, dark where not. */
function DmxPatchView({ occupancy }: { occupancy: DmxOccupancy }) {
	const [selected, setSelected] = useState<Channel | null>(null);
	const host = useRef<HTMLElement>(null);
	const columns = dmxChannelsPerRow(useWidth(host), PATCH_CELL);
	const patched = patchedUniverses(occupancy);
	const universes = patched.length ? patched : [1];

	return (
		<div className="viz-dmx-content">
			<WindowScrollArea>
				<main ref={host} style={gridStyle(columns, PATCH_CELL)}>
					{universes.map((universe) => {
						const addresses = occupancy.get(universe);
						return (
							<section
								className="viz-dmx-universe is-patch"
								key={universe}
								aria-label={`Universe ${universe} patch`}
							>
								<header>
									<b>Universe {universe}</b>
									<small>
										{addresses?.size ?? 0} of {DMX_SLOTS} channels patched
									</small>
								</header>
								{rows(columns, (address) => {
									const occupants = addresses?.get(address) ?? [];
									const isSelected =
										selected?.universe === universe &&
										selected.address === address;
									return (
										<button
											type="button"
											key={address}
											className={classes({
												"is-patched": occupants.length > 0,
												"is-start": occupants.some(
													(occupant) => occupant.channel === 1,
												),
												"is-conflict": occupants.length > 1,
												"is-selected": isSelected,
											})}
											aria-pressed={isSelected}
											aria-label={`Universe ${universe}, address ${address}, ${
												occupants.length
													? occupants.map(occupantLabel).join("; ")
													: "not patched"
											}`}
											title={
												occupants.length
													? occupants.map(occupantLabel).join("\n")
													: undefined
											}
											onClick={() => setSelected({ universe, address })}
										>
											{address}
										</button>
									);
								})}
							</section>
						);
					})}
					{patched.length ? null : (
						<p className="viz-dmx-empty">
							No fixture is patched to a DMX address yet.
						</p>
					)}
				</main>
			</WindowScrollArea>
			<aside className="viz-dmx-info">
				{selected ? (
					<ChannelInfo
						channel={selected}
						occupants={
							occupancy.get(selected.universe)?.get(selected.address) ?? []
						}
						onDeselect={() => setSelected(null)}
					/>
				) : (
					<PatchSummary occupancy={occupancy} />
				)}
			</aside>
		</div>
	);
}

/** The desk's DMX Output window, reading what arrives over Art-Net and sACN instead. */
function DmxValuesView({
	occupancy,
	dotSize,
}: {
	occupancy: DmxOccupancy;
	dotSize: DotSize;
}) {
	const received = useReceivedDmx();
	const [selected, setSelected] = useState<Channel | null>(null);
	const host = useRef<HTMLElement>(null);
	const dot = dotSize === "large" ? 42 : 9;
	const columns = dmxChannelsPerRow(useWidth(host), dot);
	const universes = received.value?.universes ?? [];
	const selectedUniverse = selected
		? universes.find((frame) => frame.universe === selected.universe)
		: undefined;

	return (
		<div className="viz-dmx-content">
			<WindowScrollArea>
				<main ref={host} style={gridStyle(columns, dot)}>
					{universes.map((frame) => (
						<section
							className={`viz-dmx-universe is-values dots-${dotSize}`}
							key={frame.universe}
							aria-label={`Universe ${frame.universe} values`}
						>
							<header>
								<b>
									Universe {frame.universe} · channels 1–{DMX_SLOTS}
								</b>
								<small>{universeState(frame)}</small>
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
	const [value, setValue] = useState<ReceivedDmx | null>(null);
	const [error, setError] = useState("");
	useEffect(() => {
		let active = true;
		let timer: number | undefined;
		const read = () => {
			documentSession
				.receivedDmx()
				.then((received) => {
					if (!active) return;
					setValue(received);
					setError("");
				})
				.catch((reason) => {
					if (active) setError(String(reason));
				})
				.finally(() => {
					// One read at a time: the next is scheduled only once this one answered.
					if (active) timer = window.setTimeout(read, 250);
				});
		};
		read();
		return () => {
			active = false;
			window.clearTimeout(timer);
			void documentSession.stopReceivedDmx().catch(() => undefined);
		};
	}, []);
	return { value, error };
}

function ChannelInfo({
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

function PatchSummary({ occupancy }: { occupancy: DmxOccupancy }) {
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

function universeState(frame: ReceivedDmx["universes"][number]): ReactNode {
	if (frame.live)
		return `${frame.protocol ? protocolLabel(frame.protocol) : "Live"} · ${frame.rateHz.toFixed(1)} Hz`;
	return frame.slots ? "Holding the last frame — source stopped" : "Waiting for DMX";
}

function protocolLabel(protocol: LiveDmxProtocol) {
	return protocol === "sacn" ? "sACN" : "Art-Net";
}

function occupantLabel(occupant: DmxOccupant) {
	const attribute = occupant.attribute ? ` ${occupant.attribute}` : "";
	return `${fixtureTitle(occupant.fixture)} channel ${occupant.channel}${attribute}`;
}

function hexAddress(address: number) {
	return `0x${address.toString(16).toUpperCase().padStart(3, "0")}`;
}

/** Rows of `columns` channels, each labelled with its first address. */
function rows(
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

function gridStyle(columns: number, cell: number) {
	return {
		"--viz-dmx-columns": columns,
		"--viz-dmx-cell": `${cell}px`,
	} as CSSProperties;
}

function classes(flags: Record<string, boolean>) {
	return Object.entries(flags)
		.filter(([, on]) => on)
		.map(([name]) => name)
		.join(" ");
}

function useWidth(host: RefObject<HTMLElement | null>) {
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

/** The dot size is this window's preference, not the show's. */
function useDotSize(): [DotSize, (size: DotSize) => void] {
	const [size, setSize] = useState<DotSize>(() => {
		try {
			return localStorage.getItem(DOT_SIZE_KEY) === "large" ? "large" : "small";
		} catch {
			return "small";
		}
	});
	return [
		size,
		(next) => {
			setSize(next);
			try {
				localStorage.setItem(DOT_SIZE_KEY, next);
			} catch {
				// A window that cannot store it still shows the size chosen now.
			}
		},
	];
}
