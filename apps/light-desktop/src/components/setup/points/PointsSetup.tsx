import { Button, NumberField, TextField } from "@tosklight/ui";
import type { TitleActionGroup } from "@tosklight/ui";
import { useEffect, useState } from "react";
import type { PsnBinding } from "../../../api/client/psn";
import type { PatchedFixture } from "../../../api/types";
import { fixtureDisplayId } from "../fixturePatch/fixtureIds";
import { formatFixturePatch } from "../fixturePatch/patchModel";
import type { PointAxis } from "./pointManagement";
import type { PointManagement } from "./usePointManagement";

const AXES: readonly PointAxis[] = ["x", "y", "z"];

/** The Points view's **+ Create Point** title action, beside the Show Patch view switch. */
export function pointsCreateGroup(points: PointManagement): TitleActionGroup {
	return {
		id: "points-actions",
		actions: [
			{
				id: "create-point",
				label: "+ Create Point",
				disabled: !points.canCreate || points.busy,
				onPress: () => void points.create(),
			},
		],
	};
}

/**
 * Show Patch › Points (TL-651): every 3D Point the Position encoders can aim at, whether it has a
 * DMX address or not and whether a tracker moves it. A Point is created without an address, named
 * and placed here; patching it stays in Fixtures, binding it to a tracker in Tracking.
 */
export function PointsSetup({ points }: { points: PointManagement }) {
	return (
		<section className="points-setup" aria-label="Points">
			<h2>Points</h2>
			<p className="points-setup-hint">
				A Point is a place on stage the Position encoders aim at. It needs no DMX
				address. Its location is where it rests; Point X/Y/Z, a tracker or a cue
				can move it from there. Patch a Point in Fixtures to send its pose over
				DMX, and bind it to a tracker in Tracking.
			</p>
			{points.message ? (
				<p
					className={points.error ? "points-setup-error" : "points-setup-status"}
					role={points.error ? "alert" : "status"}
				>
					{points.message}
				</p>
			) : null}
			{points.points.length === 0 ? (
				<p className="points-setup-empty">
					No Points yet. <b>+ Create Point</b> adds one at the stage origin.
				</p>
			) : (
				<table className="media-server-table points-table" aria-label="Points">
					<thead>
						<tr>
							<th>ID</th>
							<th>Name</th>
							<th>X</th>
							<th>Y</th>
							<th>Z</th>
							<th>Patch</th>
							<th>Tracking</th>
							<th>
								<span className="visually-hidden">Actions</span>
							</th>
						</tr>
					</thead>
					<tbody>
						{points.points.map((point) => (
							<PointRow
								key={point.fixture_id}
								point={point}
								binding={points.bindings.get(point.fixture_id)}
								created={points.createdId === point.fixture_id}
								points={points}
							/>
						))}
					</tbody>
				</table>
			)}
		</section>
	);
}

function PointRow({
	point,
	binding,
	created,
	points,
}: {
	point: PatchedFixture;
	binding: PsnBinding | undefined;
	created: boolean;
	points: PointManagement;
}) {
	const id = fixtureDisplayId(point);
	const label = `Point ${id}`;
	const [deleteArmed, setDeleteArmed] = useState(false);
	return (
		<tr
			className={created ? "is-created" : undefined}
			aria-label={`${id} · ${point.name}`}
			aria-current={created ? "true" : undefined}
		>
			<td>{id}</td>
			<td>
				<PointNameField
					label={`${label} name`}
					name={point.name ?? ""}
					disabled={points.busy}
					onCommit={(name) => void points.rename(point, name)}
				/>
			</td>
			{AXES.map((axis) => (
				<td key={axis}>
					<PointAxisField
						label={`${label} ${axis.toUpperCase()}`}
						metres={(point.location?.[axis] ?? 0) / 1000}
						disabled={points.busy}
						onCommit={(metres) => void points.move(point, axis, metres)}
					/>
				</td>
			))}
			<td>{formatFixturePatch(point)}</td>
			<td>
				{binding
					? `Tracker ${binding.trackerId}${binding.enabled ? "" : " · held by the show"}`
					: "—"}
			</td>
			<td>
				<div className="media-server-row-actions">
					{deleteArmed ? (
						<>
							<Button
								variant="danger"
								disabled={points.busy}
								onClick={() => {
									setDeleteArmed(false);
									void points.remove(point);
								}}
							>
								Confirm delete
							</Button>
							<Button onClick={() => setDeleteArmed(false)}>Keep</Button>
						</>
					) : (
						<Button
							aria-label={`Delete ${label}`}
							disabled={points.busy}
							onClick={() => setDeleteArmed(true)}
						>
							Delete
						</Button>
					)}
				</div>
			</td>
		</tr>
	);
}

function PointNameField({
	label,
	name,
	disabled,
	onCommit,
}: {
	label: string;
	name: string;
	disabled: boolean;
	onCommit(name: string): void;
}) {
	const [draft, setDraft] = useState(name);
	useEffect(() => setDraft(name), [name]);
	return (
		<TextField
			aria-label={label}
			keyboardLabel={label}
			value={draft}
			disabled={disabled}
			onValueChange={setDraft}
			onKeyboardCommit={onCommit}
			onBlur={() => onCommit(draft)}
			onKeyDown={(event) => {
				if (event.key === "Enter") event.currentTarget.blur();
			}}
		/>
	);
}

function PointAxisField({
	label,
	metres,
	disabled,
	onCommit,
}: {
	label: string;
	metres: number;
	disabled: boolean;
	onCommit(metres: number): void;
}) {
	const shown = String(Number(metres.toFixed(3)));
	const [draft, setDraft] = useState(shown);
	useEffect(() => setDraft(shown), [shown]);
	const commit = (text: string) => {
		const value = Number(text);
		if (text.trim() !== "" && Number.isFinite(value)) onCommit(value);
		else setDraft(shown);
	};
	return (
		<NumberField
			aria-label={label}
			keyboardLabel={label}
			value={draft}
			step={0.1}
			allowDecimal
			unit="m"
			disabled={disabled}
			onValueChange={setDraft}
			onStepCommit={commit}
			onKeyboardCommit={commit}
			onBlur={(event) => commit(event.currentTarget.value)}
			onKeyDown={(event) => {
				if (event.key === "Enter") event.currentTarget.blur();
			}}
		/>
	);
}
