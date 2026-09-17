// Speed Group reception, live.
//
// The Media Server only receives Speed Groups. This panel says whether a Light desk is being
// followed, what each group last said, and why anything that arrived was refused — pushed over
// the telemetry socket, because it changes many times a second.

import type {
	SpeedGroupReceptionView,
	TelemetryFrame,
} from "../../shared/api/generated/media-wire";
import { useTelemetry } from "../../shared/api/telemetry";

const CONNECTION_TEXT: Record<string, string> = {
	disabled: "Off. Enter a Speed Groups address above to follow a Light desk.",
	unavailable: "Not listening.",
	waiting: "Listening. No Light desk has sent Speed Groups yet.",
	connected: "Receiving from a Light desk.",
	lost: "Lost. The Light desk stopped sending; every group holds its last tempo.",
};

export function SpeedGroupStatus() {
	const telemetry = useTelemetry();
	return (
		<SpeedGroupReception
			reception={telemetry.frame?.speedGroups}
			connected={telemetry.connected}
		/>
	);
}

export function SpeedGroupReception({
	reception,
	connected,
}: {
	reception: TelemetryFrame["speedGroups"] | undefined;
	connected: boolean;
}) {
	if (!reception) {
		return (
			<section aria-label="Speed Group reception">
				<h3>Speed Groups</h3>
				<p className="media-state">
					{connected
						? "Reading Speed Group reception…"
						: "Not connected to this server's live status."}
				</p>
			</section>
		);
	}
	const problem =
		reception.connection === "unavailable" || reception.connection === "lost";
	return (
		<section aria-label="Speed Group reception">
			<h3>Speed Groups</h3>
			<p
				className={`media-state${problem ? " is-error" : ""}`}
				role={problem ? "alert" : "status"}
			>
				{CONNECTION_TEXT[reception.connection] ?? reception.connection}
				{reception.detail ? ` ${reception.detail}` : ""}
			</p>
			<ReceptionFacts reception={reception} />
			{reception.groups.length > 0 && (
				<table className="media-table">
					<caption>Received Speed Groups</caption>
					<thead>
						<tr>
							<th scope="col">Group</th>
							<th scope="col">BPM</th>
							<th scope="col">State</th>
						</tr>
					</thead>
					<tbody>
						{reception.groups.map((group) => (
							<tr key={group.group}>
								<th scope="row">{group.group}</th>
								<td>{group.bpm.toFixed(1)}</td>
								<td>
									{!group.fresh
										? "Stale, holding"
										: group.running
											? "Running"
											: "Paused"}
								</td>
							</tr>
						))}
					</tbody>
				</table>
			)}
			{reception.rejections.length > 0 && (
				<>
					<h4>Refused messages</h4>
					<ul className="media-settings-note">
						{reception.rejections.map((rejection) => (
							<li key={`${rejection.ageMillis}-${rejection.reason}`}>
								{rejection.from ? `${rejection.from}: ` : ""}
								{rejection.reason} ({formatAge(rejection.ageMillis)} ago)
							</li>
						))}
					</ul>
				</>
			)}
		</section>
	);
}

function ReceptionFacts({ reception }: { reception: SpeedGroupReceptionView }) {
	return (
		<dl className="media-facts">
			<dt>Listening on</dt>
			<dd>
				<code>{reception.listening ?? "—"}</code>
			</dd>
			<dt>Light desk</dt>
			<dd>
				{reception.sender
					? `${reception.sender} (${reception.senderAddress ?? "unknown address"})`
					: "—"}
			</dd>
			<dt>Last update</dt>
			<dd>
				{reception.lastUpdateAgeMillis === null
					? "never"
					: `${formatAge(reception.lastUpdateAgeMillis)} ago`}
			</dd>
			<dt>Messages</dt>
			<dd>
				{reception.accepted} accepted, {reception.rejected} refused
			</dd>
		</dl>
	);
}

function formatAge(millis: number): string {
	return millis < 1_000
		? `${millis} ms`
		: `${(millis / 1_000).toFixed(millis < 10_000 ? 1 : 0)} s`;
}
