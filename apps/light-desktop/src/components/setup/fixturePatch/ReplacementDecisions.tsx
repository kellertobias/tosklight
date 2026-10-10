import { Button, CheckboxField, Select } from "@tosklight/ui";
import { useRef } from "react";
import type { PatchController } from "./controller";
import {
	rootProgrammingCorrespondences,
	rootProgrammingDecision,
} from "./replacementProgramming";

export interface ReplacementDecisionRow {
	key: string;
	source: string;
	multiple: boolean;
	targets: Array<{ id: string; name: string }>;
	choice: string;
	state: "required" | "invalid" | "dormant" | "mapped";
}

/** Derived presentation only; the save controller remains the consent authority. */
export function replacementDecisionRows(
	controller: PatchController,
): ReplacementDecisionRow[] {
	const { selected, definition } = controller.data;
	const choices = controller.ui.replacementHeads;
	const shared = rootProgrammingCorrespondences(
		selected?.definition,
		definition,
	).map((row) => {
		const choice = choices[row.key] ?? "";
		return {
			key: row.key,
			source: `Shared head ${row.sourceName} · ${row.attribute}`,
			multiple: true,
			targets: row.targets,
			choice,
			state: !choice
				? "required"
				: !rootProgrammingDecision(row, choice)
					? "invalid"
					: choice === "__unmapped"
						? "dormant"
						: "mapped",
		} as ReplacementDecisionRow;
	});
	const targets = (
		definition?.profile_snapshot?.modes.find(
			(mode) => mode.id === definition.mode_id,
		)?.heads ?? []
	)
		.map((head, index) => ({ head, index }))
		.filter(({ head }) => !head.master_shared)
		.map(({ head, index }) => ({
			id: head.id,
			name: `${head.name || "Head"} · ${index + 1}`,
		}));
	return [
		...shared,
		...(selected?.logical_heads ?? []).map((head) => {
			const choice = choices[head.fixture_id] ?? "";
			return {
				key: head.fixture_id,
				source: `Existing head ${head.head_index + 1}`,
				multiple: false,
				targets,
				choice,
				state: !choice
					? "required"
					: choice === "__unmapped"
						? "dormant"
						: targets.some((target) => target.id === choice)
							? "mapped"
							: "invalid",
			} as ReplacementDecisionRow;
		}),
	];
}

export function ReplacementDecisions({
	rows,
	onChoice,
	disabled,
}: {
	rows: ReplacementDecisionRow[];
	onChoice(key: string, value: string): void;
	disabled: boolean;
}) {
	const container = useRef<HTMLDivElement>(null);
	const unresolved = rows.filter(
		(row) => row.state === "required" || row.state === "invalid",
	);
	const next = () => {
		const row = [
			...(container.current?.querySelectorAll<HTMLElement>(
				"[data-decision-key]",
			) ?? []),
		].find((element) => element.dataset.decisionKey === unresolved[0]?.key);
		row?.scrollIntoView({ block: "nearest" });
		row
			?.querySelector<HTMLElement>(
				'input:not(:disabled), button[aria-haspopup="listbox"]:not(:disabled)',
			)
			?.focus();
	};
	return (
		<div ref={container} className="import-workflow__decisions">
			<div className="import-decision-summary">
				<h3>Required correspondences</h3>
				<p role="status">
					{unresolved.length} of {rows.length} decisions remaining ·{" "}
					{rows.filter((row) => row.multiple).length} shared families ·{" "}
					{rows.filter((row) => !row.multiple).length} logical heads
				</p>
				<Button disabled={disabled || !unresolved.length} onClick={next}>
					Next unresolved
				</Button>
			</div>
			<p>
				Choose each source's destination owners, or explicitly keep it dormant.
				Shared families can route to several owners. New master-only edits
				retain their normal meaning.
			</p>
			{rows.map((row) => (
				<DecisionRow
					key={row.key}
					row={row}
					disabled={disabled}
					onChoice={(value) => onChoice(row.key, value)}
				/>
			))}
		</div>
	);
}

function DecisionRow({
	row,
	disabled,
	onChoice,
}: {
	row: ReplacementDecisionRow;
	disabled: boolean;
	onChoice(value: string): void;
}) {
	const destinations =
		row.choice === "__unmapped" ? [] : row.choice.split(",").filter(Boolean);
	const required = row.state === "required" || row.state === "invalid";
	return (
		<fieldset
			aria-label={row.source}
			className="import-mapping-row"
			data-state={row.state}
			data-decision-key={row.key}
		>
			<div className="import-mapping-row__source">
				<strong>{row.source}</strong>
			</div>
			<div className="import-mapping-row__target">
				<strong>Destination →</strong>
				{row.multiple ? (
					<>
						<CheckboxField
							label="Leave unmatched — keep dormant programming"
							disabled={disabled}
							aria-label={`Leave ${row.source} unmatched`}
							checked={row.choice === "__unmapped"}
							onChange={(event) =>
								onChoice(event.target.checked ? "__unmapped" : "")
							}
						/>
						{row.targets.map((target) => (
							<CheckboxField
								key={target.id}
								label={target.name}
								disabled={disabled}
								aria-label={`Route ${row.source} to ${target.name}`}
								checked={destinations.includes(target.id)}
								onChange={(event) =>
									onChoice(
										(event.target.checked
											? [...destinations, target.id]
											: destinations.filter((id) => id !== target.id)
										).join(","),
									)
								}
							/>
						))}
					</>
				) : (
					<Select
						disabled={disabled}
						aria-label={`Replacement for ${row.source.toLowerCase().replace("existing ", "")}`}
						value={row.choice}
						onChange={(event) => onChoice(event.target.value)}
					>
						<option value="">Choose correspondence</option>
						<option value="__unmapped">
							Leave unmatched — keep dormant programming
						</option>
						{row.targets.map((target) => (
							<option key={target.id} value={target.id}>
								{target.name}
							</option>
						))}
					</Select>
				)}
				{!row.targets.length && (
					<p>
						No compatible destination for this family. Explicitly leave
						unmatched to retain dormant programming.
					</p>
				)}
			</div>
			<p
				className="import-mapping-row__state"
				role={row.state === "invalid" ? "alert" : undefined}
			>
				{row.state === "invalid"
					? "This correspondence is no longer compatible. Choose again."
					: row.state === "dormant"
						? "Resolved · programming stays stored and dormant"
						: row.state === "mapped"
							? "Resolved · destination selected"
							: "Required · choose destination or dormant"}
			</p>
			{required && (
				<small>Set remains unavailable until this decision is resolved.</small>
			)}
		</fieldset>
	);
}
