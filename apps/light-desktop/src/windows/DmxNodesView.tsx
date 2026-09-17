import { Button } from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import { useState } from "react";
import type {
	NetworkEndpoint,
	NetworkEndpointStatus,
	NetworkEndpointsSnapshot,
} from "../api/generated/light-wire";
import "./DmxNodesView.css";

const STATUS_LABEL: Record<NetworkEndpointStatus, string> = {
	active: "Active",
	listening: "Listening",
	idle: "Idle",
	disabled: "Disabled",
	conflict: "Conflict",
	error: "Error",
	unavailable: "Unavailable",
};

/** Statuses the operator has to act on, listed first in the summary. */
const ATTENTION: readonly NetworkEndpointStatus[] = [
	"error",
	"conflict",
	"unavailable",
];

function protocolLabel(endpoint: NetworkEndpoint): string {
	return endpoint.protocol === "art_net" ? "Art-Net" : "sACN";
}

function deliveryLabel(endpoint: NetworkEndpoint): string | null {
	switch (endpoint.delivery_mode) {
		case "broadcast":
			return "Broadcast";
		case "multicast":
			return "Multicast";
		case "unicast":
			return "Unicast";
		default:
			return null;
	}
}

/** Universes as compact ranges: `1–4, 7`. */
export function universeRanges(universes: readonly number[]): string {
	const sorted = [...new Set(universes)].sort((left, right) => left - right);
	const ranges: string[] = [];
	let start = sorted[0];
	let previous = sorted[0];
	for (const universe of [...sorted.slice(1), Number.NaN]) {
		if (universe === previous + 1) {
			previous = universe;
			continue;
		}
		if (start !== undefined) {
			ranges.push(start === previous ? `${start}` : `${start}–${previous}`);
		}
		start = universe;
		previous = universe;
	}
	return ranges.join(", ");
}

function universeCell(endpoint: NetworkEndpoint): string {
	if (!endpoint.universes.length) return "—";
	const wire = universeRanges(endpoint.universes);
	return endpoint.logical_universe == null
		? wire
		: `${endpoint.logical_universe} → ${wire}`;
}

function lastActivity(millis: number | null): string {
	if (millis == null) return "—";
	if (millis < 1_000) return "now";
	return `${Math.round(millis / 1_000)} s ago`;
}

export interface DmxNodesViewProps {
	/** `null` until the first answer arrives. */
	snapshot: NetworkEndpointsSnapshot | null;
	/** The last read failure, cleared by the next successful read. */
	error: string | null;
	/** False where no live server can be asked, such as a static preview. */
	supported: boolean;
}

/**
 * The DMX screen's Nodes tab: every Art-Net and sACN endpoint the desk sends to or hears from.
 * The server decides every status; this view only lays them out.
 */
export function DmxNodesView({
	snapshot,
	error,
	supported,
}: DmxNodesViewProps) {
	const [selectedId, setSelectedId] = useState<string | null>(null);
	const endpoints = snapshot?.endpoints ?? [];
	const selected = endpoints.find((endpoint) => endpoint.id === selectedId);
	return (
		<div className="dmx-content">
			<WindowScrollArea>
				<main className="dmx-nodes">
					{error ? (
						<div className="dmx-nodes-error" role="alert">
							<b>Network state could not be read.</b>
							<span>{error}</span>
							<small>
								The list shows the last known state and refreshes when the desk
								answers again.
							</small>
						</div>
					) : null}
					{endpoints.length ? (
						<NodesTable
							endpoints={endpoints}
							selectedId={selectedId}
							onSelect={setSelectedId}
						/>
					) : (
						<div className="empty-window-message">
							{!supported
								? "Network nodes are available when the desk is connected to its server."
								: snapshot
									? "No Art-Net or sACN endpoint. Add an output route in Desk Setup > Outputs > Routes to send DMX."
									: error
										? "No network state yet."
										: "Reading network state…"}
						</div>
					)}
				</main>
			</WindowScrollArea>
			<aside className="dmx-info-pane dmx-nodes-pane">
				{selected ? (
					<EndpointInfo
						endpoint={selected}
						onDeselect={() => setSelectedId(null)}
					/>
				) : (
					<NodesSummary snapshot={snapshot} />
				)}
			</aside>
		</div>
	);
}

function NodesTable({
	endpoints,
	selectedId,
	onSelect,
}: {
	endpoints: readonly NetworkEndpoint[];
	selectedId: string | null;
	onSelect: (id: string | null) => void;
}) {
	return (
		<table className="dmx-nodes-table" aria-label="Network nodes">
			<thead>
				<tr>
					<th>Protocol</th>
					<th>Direction</th>
					<th>Endpoint</th>
					<th>Universe</th>
					<th>Status</th>
				</tr>
			</thead>
			<tbody>
				{endpoints.map((endpoint) => {
					const isSelected = endpoint.id === selectedId;
					const delivery = deliveryLabel(endpoint);
					return (
						<tr
							key={endpoint.id}
							className={isSelected ? "selected" : undefined}
							data-endpoint-id={endpoint.id}
						>
							<td>
								<b>{protocolLabel(endpoint)}</b>
								{delivery ? <small>{delivery}</small> : null}
							</td>
							<td>
								<span className={`dmx-nodes-direction ${endpoint.direction}`}>
									{endpoint.direction === "send" ? "Send" : "Receive"}
								</span>
								<small>
									{endpoint.origin === "observed" ? "Heard" : "Configured"}
								</small>
							</td>
							<td>
								<button
									type="button"
									aria-pressed={isSelected}
									onClick={() => onSelect(isSelected ? null : endpoint.id)}
								>
									<code>{endpoint.endpoint}</code>
								</button>
								<small>
									{endpoint.name
										? `${endpoint.role} · ${endpoint.name}`
										: endpoint.role}
								</small>
							</td>
							<td>
								<code>{universeCell(endpoint)}</code>
							</td>
							<td>
								<span className={`dmx-nodes-status status-${endpoint.status}`}>
									{STATUS_LABEL[endpoint.status]}
								</span>
								<small>{endpoint.detail}</small>
							</td>
						</tr>
					);
				})}
			</tbody>
		</table>
	);
}

function EndpointInfo({
	endpoint,
	onDeselect,
}: {
	endpoint: NetworkEndpoint;
	onDeselect: () => void;
}) {
	return (
		<>
			<header className="dmx-info-header">
				<b>Selected endpoint</b>
				<Button size="compact" onClick={onDeselect}>
					Deselect
				</Button>
			</header>
			<section className="dmx-fixture-card">
				<b>{endpoint.role}</b>
				<dl>
					<dt>Protocol</dt>
					<dd>{protocolLabel(endpoint)}</dd>
					<dt>Direction</dt>
					<dd>{endpoint.direction === "send" ? "Send" : "Receive"}</dd>
					<dt>Origin</dt>
					<dd>
						{endpoint.origin === "observed"
							? "Heard on the network"
							: "Configured"}
					</dd>
					<dt>Endpoint</dt>
					<dd>{endpoint.endpoint}</dd>
					<dt>Name</dt>
					<dd>{endpoint.name ?? "—"}</dd>
					<dt>Delivery</dt>
					<dd>{deliveryLabel(endpoint) ?? "—"}</dd>
					<dt>Logical universe</dt>
					<dd>{endpoint.logical_universe ?? "—"}</dd>
					<dt>Wire universes</dt>
					<dd>
						{endpoint.universes.length
							? universeRanges(endpoint.universes)
							: "—"}
					</dd>
					<dt>Last activity</dt>
					<dd>{lastActivity(endpoint.last_activity_millis_ago)}</dd>
					<dt>Send errors</dt>
					<dd>{endpoint.errors}</dd>
				</dl>
			</section>
			<section>
				<b>Status · {STATUS_LABEL[endpoint.status]}</b>
				<p className={`dmx-nodes-detail status-${endpoint.status}`}>
					{endpoint.detail}
				</p>
			</section>
		</>
	);
}

function NodesSummary({
	snapshot,
}: {
	snapshot: NetworkEndpointsSnapshot | null;
}) {
	const endpoints = snapshot?.endpoints ?? [];
	const count = (predicate: (endpoint: NetworkEndpoint) => boolean) =>
		endpoints.filter(predicate).length;
	const attention = endpoints.filter((endpoint) =>
		ATTENTION.includes(endpoint.status),
	);
	return (
		<>
			<b>Network summary</b>
			<section>
				<b>Output interface</b>
				<p>
					{snapshot
						? `${snapshot.output_bind_ip} · ${
								snapshot.network_output_available
									? "network output running"
									: "network output not running"
							}`
						: "—"}
				</p>
			</section>
			<section>
				<b>Endpoints</b>
				<dl className="dmx-nodes-counts">
					<dt>Art-Net send</dt>
					<dd>
						{count((e) => e.protocol === "art_net" && e.direction === "send")}
					</dd>
					<dt>Art-Net receive</dt>
					<dd>
						{count(
							(e) => e.protocol === "art_net" && e.direction === "receive",
						)}
					</dd>
					<dt>sACN send</dt>
					<dd>
						{count((e) => e.protocol === "sacn" && e.direction === "send")}
					</dd>
					<dt>sACN receive</dt>
					<dd>
						{count((e) => e.protocol === "sacn" && e.direction === "receive")}
					</dd>
				</dl>
			</section>
			<section>
				<b>Needs attention</b>
				{attention.length ? (
					<ul className="dmx-nodes-attention">
						{attention.map((endpoint) => (
							<li key={endpoint.id}>
								<span className={`dmx-nodes-status status-${endpoint.status}`}>
									{STATUS_LABEL[endpoint.status]}
								</span>{" "}
								{protocolLabel(endpoint)} {endpoint.endpoint}
							</li>
						))}
					</ul>
				) : (
					<p>Nothing needs attention.</p>
				)}
			</section>
		</>
	);
}
