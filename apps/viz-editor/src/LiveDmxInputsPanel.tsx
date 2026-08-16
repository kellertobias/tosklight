import { Button, SelectField, SwitchField } from "@tosklight/ui";
import { useEffect, useMemo, useState } from "react";
import {
	type DeskPeer,
	type DocumentSummary,
	documentSession,
	type LiveDmxInputMapping,
	type LiveDmxInputs,
	type LiveDmxProtocol,
} from "./document/session";

const EMPTY: LiveDmxInputs = { schemaVersion: 1, mappings: [] };

export function LiveDmxInputsPanel({
	document,
	desks,
	onError,
}: {
	document: DocumentSummary;
	desks: readonly DeskPeer[];
	onError: (reason: unknown) => void;
}) {
	const [saved, setSaved] = useState<LiveDmxInputs>(EMPTY);
	const [draft, setDraft] = useState<LiveDmxInputs>(EMPTY);
	const [busy, setBusy] = useState(false);
	const [status, setStatus] = useState("");
	const [desk, setDesk] = useState("");

	useEffect(() => {
		let current = true;
		documentSession
			.liveDmxInputs()
			.then((inputs) => {
				if (!current) return;
				setSaved(inputs);
				setDraft(inputs);
			})
			.catch(onError);
		return () => {
			current = false;
		};
	}, [document.showId, onError]);

	useEffect(() => {
		if (!desks.some((candidate) => candidate.instance === desk))
			setDesk(desks[0]?.instance ?? "");
	}, [desk, desks]);

	const validation = useMemo(() => validate(draft), [draft]);
	const changed = JSON.stringify(saved) !== JSON.stringify(draft);

	function update(id: string, change: Partial<LiveDmxInputMapping>) {
		setDraft((current) => ({
			...current,
			mappings: current.mappings.map((mapping) =>
				mapping.id === id ? { ...mapping, ...change } : mapping,
			),
		}));
		setStatus("");
	}

	async function apply() {
		setBusy(true);
		setStatus("Applying live DMX inputs…");
		try {
			const applied = await documentSession.saveLiveDmxInputs(draft);
			setSaved(applied);
			setDraft(applied);
			setStatus(
				`Applied ${applied.mappings.length} live DMX input${applied.mappings.length === 1 ? "" : "s"}.`,
			);
		} catch (reason) {
			setStatus("");
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	async function takeFromDesk() {
		const source =
			desks.find((candidate) => candidate.instance === desk) ?? desks[0];
		if (!source) return;
		setBusy(true);
		setStatus(`Reading output routes from ${source.name}…`);
		try {
			const preview = await documentSession.takeLiveDmxInputsFromDesk(
				source.instance,
			);
			setDraft(preview);
			setStatus(
				`Previewing ${preview.mappings.length} compatible mapping${preview.mappings.length === 1 ? "" : "s"} from ${source.name}. Review them, then Apply.`,
			);
		} catch (reason) {
			setStatus("");
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	return (
		<section
			className="viz-live-inputs"
			aria-labelledby="live-dmx-inputs-title"
		>
			<header>
				<div>
					<h2 id="live-dmx-inputs-title">Live DMX Inputs</h2>
					<p>
						Real Art-Net or sACN received by the separate Visualizer output.
					</p>
				</div>
				<div className="viz-live-input-actions">
					{desks.length ? (
						<>
							{desks.length > 1 ? (
								<label>
									Desk
									<select
										value={desk}
										onChange={(event) => setDesk(event.target.value)}
									>
										{desks.map((candidate) => (
											<option
												key={candidate.instance}
												value={candidate.instance}
											>
												{candidate.name} · {candidate.show ?? "No show"}
											</option>
										))}
									</select>
								</label>
							) : null}
							<Button disabled={busy} onClick={() => void takeFromDesk()}>
								Take from Desk ·{" "}
								{desks.find((candidate) => candidate.instance === desk)?.name ??
									desks[0].name}
							</Button>
						</>
					) : null}
					<Button disabled={busy} onClick={() => setDraft(addMapping(draft))}>
						Add input
					</Button>
				</div>
			</header>

			{draft.mappings.length ? (
				<div className="viz-live-input-table-wrap">
					<table>
						<thead>
							<tr>
								<th className="viz-live-show-universe">Show universe</th>
								<th>Protocol</th>
								<th>Wire universe</th>
								<th>Delivery</th>
								<th>UDP port</th>
								<th>
									<span className="sr-only">Actions</span>
								</th>
								<th className="viz-live-enabled">Enabled</th>
							</tr>
						</thead>
						<tbody>
							{draft.mappings.map((mapping) => (
								<InputRow
									key={mapping.id}
									mapping={mapping}
									update={update}
									remove={() =>
										setDraft((current) => ({
											...current,
											mappings: current.mappings.filter(
												(candidate) => candidate.id !== mapping.id,
											),
										}))
									}
								/>
							))}
						</tbody>
					</table>
				</div>
			) : (
				<p className="viz-live-input-empty">
					No explicit inputs. The renderer follows compatible show routes, then
					safe defaults.
				</p>
			)}

			{validation ? (
				<output className="viz-live-input-error" role="alert">
					{validation}
				</output>
			) : null}
			{status ? <output className="viz-editor-status">{status}</output> : null}
			<footer>
				<Button
					disabled={busy || !changed}
					onClick={() => {
						setDraft(saved);
						setStatus("Changes cancelled.");
					}}
				>
					Cancel
				</Button>
				<Button
					disabled={busy || !changed || Boolean(validation)}
					onClick={() => void apply()}
				>
					Apply
				</Button>
			</footer>
		</section>
	);
}

function InputRow({
	mapping,
	update,
	remove,
}: {
	mapping: LiveDmxInputMapping;
	update: (id: string, change: Partial<LiveDmxInputMapping>) => void;
	remove: () => void;
}) {
	const protocolChange = (protocol: LiveDmxProtocol) =>
		update(mapping.id, {
			protocol,
			port: protocol === "sacn" ? 5568 : 6454,
			delivery: protocol === "sacn" ? "multicast" : "broadcast",
			destinationUniverse: Math.max(
				protocol === "sacn" ? 1 : 0,
				mapping.destinationUniverse,
			),
		});
	return (
		<tr className={mapping.enabled ? undefined : "is-disabled"}>
			<td className="viz-live-show-universe">
				<NumberField
					label="Show universe"
					value={mapping.logicalUniverse}
					onChange={(logicalUniverse) =>
						update(mapping.id, { logicalUniverse })
					}
				/>
			</td>
			<td>
				<SelectField
					ariaLabel="Protocol"
					value={mapping.protocol}
					onChange={protocolChange}
					options={[
						{ value: "artnet", label: "Art-Net" },
						{ value: "sacn", label: "Streaming ACN (sACN)" },
					]}
				/>
			</td>
			<td>
				<NumberField
					label="Wire universe"
					value={mapping.destinationUniverse}
					onChange={(destinationUniverse) =>
						update(mapping.id, { destinationUniverse })
					}
				/>
			</td>
			<td>
				<SelectField
					ariaLabel="Delivery"
					value={mapping.delivery}
					onChange={(delivery) => update(mapping.id, { delivery })}
					options={[
						mapping.protocol === "artnet"
							? { value: "broadcast", label: "Broadcast" }
							: { value: "multicast", label: "Multicast" },
						{ value: "unicast", label: "Unicast" },
					]}
				/>
			</td>
			<td>
				<NumberField
					label="UDP port"
					value={mapping.port}
					onChange={(port) => update(mapping.id, { port })}
				/>
			</td>
			<td>
				<Button variant="danger" onClick={remove}>
					Remove
				</Button>
			</td>
			<td className="viz-live-enabled">
				<SwitchField
					label="Enabled"
					aria-label={`Enable universe ${mapping.logicalUniverse}`}
					offLabel={null}
					onLabel={null}
					controlOnly
					checked={mapping.enabled}
					onChange={(event) =>
						update(mapping.id, { enabled: event.target.checked })
					}
				/>
			</td>
		</tr>
	);
}

function NumberField({
	label,
	value,
	onChange,
}: {
	label: string;
	value: number;
	onChange: (value: number) => void;
}) {
	return (
		<input
			aria-label={label}
			type="number"
			value={value}
			onChange={(event) => onChange(Number(event.target.value))}
		/>
	);
}

function addMapping(inputs: LiveDmxInputs): LiveDmxInputs {
	const used = new Set(
		inputs.mappings.map((mapping) => mapping.logicalUniverse),
	);
	let logicalUniverse = 1;
	while (used.has(logicalUniverse) && logicalUniverse < 65_535)
		logicalUniverse += 1;
	return {
		...inputs,
		mappings: [
			...inputs.mappings,
			{
				id: crypto.randomUUID(),
				logicalUniverse,
				protocol: "artnet",
				destinationUniverse: logicalUniverse,
				port: 6454,
				enabled: true,
				delivery: "broadcast",
			},
		],
	};
}

export function validate(inputs: LiveDmxInputs): string {
	const universes = new Set<number>();
	for (const mapping of inputs.mappings) {
		if (
			!Number.isInteger(mapping.logicalUniverse) ||
			mapping.logicalUniverse < 1 ||
			mapping.logicalUniverse > 65_535
		)
			return "Show universes must be whole numbers from 1 to 65535.";
		if (universes.has(mapping.logicalUniverse))
			return `Show universe ${mapping.logicalUniverse} is configured more than once.`;
		universes.add(mapping.logicalUniverse);
		const maximum = mapping.protocol === "sacn" ? 63_999 : 32_767;
		const minimum = mapping.protocol === "sacn" ? 1 : 0;
		if (
			!Number.isInteger(mapping.destinationUniverse) ||
			mapping.destinationUniverse < minimum ||
			mapping.destinationUniverse > maximum
		)
			return `${mapping.protocol === "sacn" ? "sACN" : "Art-Net"} wire universes must be from ${minimum} to ${maximum}.`;
		if (
			!Number.isInteger(mapping.port) ||
			mapping.port < 1 ||
			mapping.port > 65_535
		)
			return "UDP ports must be whole numbers from 1 to 65535.";
		if (mapping.protocol === "artnet" && mapping.delivery === "multicast")
			return "Art-Net inputs use Broadcast or Unicast delivery.";
		if (mapping.protocol === "sacn" && mapping.delivery === "broadcast")
			return "sACN inputs use Multicast or Unicast delivery.";
	}
	return "";
}
