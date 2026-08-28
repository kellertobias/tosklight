import {
	Button,
	FormLayout,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import { useCallback, useEffect, useState } from "react";
import type {
	PsnBinding,
	PsnEdit,
	PsnSnapshot,
	PsnZone,
} from "../../api/client/psn";
import { usePsn } from "../../features/psn/PsnContext";
import { PsnBindingList } from "./PsnBindingList";
import { PsnCalibrationForm } from "./PsnCalibrationForm";
import { PsnSourceForm } from "./PsnSourceForm";
import { PsnTrackerTable } from "./PsnTrackerTable";
import { PsnZoneEditor } from "./PsnZoneEditor";

/** How often the tab asks again while it is open. */
const REFRESH_MILLIS = 500;

/**
 * Tracking, as the Show Patch shows it.
 *
 * Three things in one page, in the order an operator meets them: is anything arriving, what is
 * arriving, and what it drives. Nothing here decides show semantics — which fixtures are 3D
 * Points, whether a position is stale, where a point ends up — all of that is the desk's answer,
 * read from one request.
 */
export function PsnSetup({ active = true }: { active?: boolean }) {
	const psn = usePsn();
	const [snapshot, setSnapshot] = useState<PsnSnapshot | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);

	const refresh = useCallback(async () => {
		if (!psn) return;
		try {
			setSnapshot(await psn.snapshot());
		} catch (cause) {
			setError(cause instanceof Error ? cause.message : String(cause));
		}
	}, [psn]);

	useEffect(() => {
		if (!active) return;
		void refresh();
		const timer = window.setInterval(() => void refresh(), REFRESH_MILLIS);
		return () => window.clearInterval(timer);
	}, [active, refresh]);

	const edit = async (change: PsnEdit) => {
		if (!psn) return;
		setBusy(true);
		setError(null);
		try {
			await psn.update(change);
			await refresh();
		} catch (cause) {
			setError(cause instanceof Error ? cause.message : String(cause));
		} finally {
			setBusy(false);
		}
	};

	if (!snapshot) {
		return (
			<section className="psn-setup">
				<h2>Tracking</h2>
				<p>{error ?? "Reading the tracking configuration…"}</p>
			</section>
		);
	}

	const { configuration, status, points, macros } = snapshot;
	const bind = (trackerId: number, pointFixtureId: string) =>
		edit({
			bindings: [
				...configuration.bindings.filter(
					(binding) => binding.trackerId !== trackerId,
				),
				{
					id: crypto.randomUUID(),
					trackerId,
					pointFixtureId,
					enabled: true,
				},
			],
		});
	return (
		<section className="psn-setup">
			<h2>Tracking</h2>
			<p className="psn-status" role="status">
				{describeStatus(snapshot)}
			</p>
			{status.error && (
				<p className="psn-error" role="alert">
					{status.error}
				</p>
			)}
			{error && (
				<p className="psn-error" role="alert">
					{error}
				</p>
			)}

			<PsnSourceForm configuration={configuration} busy={busy} onEdit={edit} />

			<h3>Trackers</h3>
			<PsnTrackerTable
				trackers={status.trackers}
				bindings={configuration.bindings}
				points={points}
				busy={busy}
				onBind={(trackerId, pointFixtureId) =>
					void bind(trackerId, pointFixtureId)
				}
			/>

			<h3>Bound points</h3>
			<PsnBindingList
				bindings={configuration.bindings}
				points={points}
				placements={status.placements}
				busy={busy}
				onEdit={edit}
			/>

			<h3>Zones</h3>
			<PsnZoneEditor
				zones={configuration.zones}
				macros={macros}
				occupiedZoneIds={status.occupiedZoneIds}
				busy={busy}
				onChange={(zones: PsnZone[]) => void edit({ zones })}
			/>

			<PsnCalibrationForm configuration={configuration} onEdit={edit} />
		</section>
	);
}

function describeStatus(snapshot: PsnSnapshot): string {
	const { status } = snapshot;
	if (!status.enabled) return "Not listening. Tracking is switched off.";
	const where = status.listeningOn ?? "the configured group";
	const senders = status.systemNames.length
		? ` from ${status.systemNames.join(", ")}`
		: "";
	switch (status.health?.state) {
		case "receiving":
			return `Receiving on ${where}${senders} — ${status.trackers.length} tracker(s), ${status.frames} frames.`;
		case "stale":
			return `Nothing heard on ${where} for ${Math.round(status.health.silentForMillis / 1000)}s. Bound points are holding their last position.`;
		default:
			return `Listening on ${where}. Nothing has arrived yet — the sender may be off, or on another network.`;
	}
}

