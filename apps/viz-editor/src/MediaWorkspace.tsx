import { open } from "@tauri-apps/plugin-dialog";
import {
	DmxAddressField,
	FixtureAddFlow,
	FixtureAddressFlow,
	usePatchFixture,
} from "@tosklight/patch";
import { Button } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { useEffect, useMemo, useState } from "react";
import {
	documentSession,
	type LedModuleType,
	type MediaLayoutSnapshot,
	type MediaObject,
	type MediaProjector,
	type MediaServer,
	type MediaSource,
	type MediaSurface,
	type MediaSurfaceSection,
	type VersionedMediaObject,
} from "./document/session";
import { beginWindowDrag } from "./WindowChrome";

const EMPTY: MediaLayoutSnapshot = {
	fallbackAssets: [],
	servers: [],
	sources: [],
	ledModuleTypes: [],
	surfaces: [],
	projectors: [],
};

type WorkspaceTab = "servers" | "surfaces" | "modules" | "projectors";

function id() {
	return crypto.randomUUID();
}

function transform() {
	return {
		positionMetres: [0, 1, 0] as [number, number, number],
		rotationDegrees: [0, 0, 0] as [number, number, number],
	};
}

function label(entry: VersionedMediaObject) {
	return entry.object.body.name;
}

function NumberInput({
	value,
	onChange,
	min,
	max,
	step = 0.01,
}: {
	value: number;
	onChange: (value: number) => void;
	min?: number;
	max?: number;
	step?: number;
}) {
	return (
		<input
			type="number"
			value={value}
			min={min}
			max={max}
			step={step}
			onChange={(event) => onChange(Number(event.target.value))}
		/>
	);
}

export function MediaWorkspace({
	onError,
}: {
	onError: (reason: unknown) => void;
}) {
	const [layout, setLayout] = useState(EMPTY);
	const [tab, setTab] = useState<WorkspaceTab>("servers");
	const [selected, setSelected] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [addServerRequest, setAddServerRequest] = useState(0);
	const [addressRequest, setAddressRequest] = useState(0);
	const [serverToLink, setServerToLink] = useState<string | null>(null);

	useEffect(() => {
		documentSession.mediaLayout().then(setLayout).catch(onError);
	}, [onError]);

	const entries = useMemo(() => {
		switch (tab) {
			case "servers":
				return layout.servers;
			case "surfaces":
				return layout.surfaces;
			case "modules":
				return layout.ledModuleTypes;
			case "projectors":
				return layout.projectors;
		}
	}, [layout, tab]);
	const current =
		entries.find((entry) => entry.object.body.id === selected) ?? entries[0];
	const linkedFixtureId =
		current?.object.kind === "media_server"
			? (current.object.body.fixtureId ?? null)
			: null;
	const linkedFixture = usePatchFixture(linkedFixtureId);

	async function apply(object: MediaObject, revision: number) {
		setBusy(true);
		try {
			const outcome = await documentSession.applyMediaIntent({
				requestId: id(),
				expectedRevision: revision,
				action: { type: "put", object },
			});
			setLayout(outcome.snapshot);
			setSelected(object.body.id);
		} catch (reason) {
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	async function enumerateOutputs() {
		const selectedServer =
			layout.servers.find((entry) => entry.object.body.id === selected)
				?.object ?? layout.servers[0]?.object;
		if (!selectedServer || selectedServer.kind !== "media_server") return;
		setBusy(true);
		try {
			const outputs = await documentSession.inspectCitpServer(
				selectedServer.body.citp.host,
				selectedServer.body.citp.port,
			);
			let snapshot = layout;
			for (const output of outputs) {
				const existing = snapshot.sources.find(
					(entry) =>
						entry.object.kind === "media_source" &&
						entry.object.body.serverId === selectedServer.body.id &&
						entry.object.body.advertisedSourceId === output.id,
				);
				const source: MediaSource = {
					id: existing?.object.body.id ?? id(),
					serverId: selectedServer.body.id,
					advertisedSourceId: output.id,
					name: output.name,
					outputName: output.name,
					width: output.width,
					height: output.height,
					aspectRatio: output.height ? output.width / output.height : null,
				};
				const outcome = await documentSession.applyMediaIntent({
					requestId: id(),
					expectedRevision: existing?.revision ?? 0,
					action: {
						type: "put",
						object: { kind: "media_source", body: source },
					},
				});
				snapshot = outcome.snapshot;
			}
			setLayout(snapshot);
		} catch (reason) {
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	async function discoverServers() {
		setBusy(true);
		try {
			const discovered = await documentSession.discoverCitpServers();
			let snapshot = layout;
			for (const peer of discovered) {
				const existing = snapshot.servers.find(
					(entry) =>
						entry.object.kind === "media_server" &&
						entry.object.body.citp.discoveryIdentity === peer.name,
				);
				const server: MediaServer = {
					id: existing?.object.body.id ?? id(),
					name: peer.name,
					citp: {
						host: peer.host,
						port: peer.port,
						discoveryIdentity: peer.name,
					},
					lastKnownEndpoint: `${peer.host}:${peer.port}`,
				};
				const outcome = await documentSession.applyMediaIntent({
					requestId: id(),
					expectedRevision: existing?.revision ?? 0,
					action: {
						type: "put",
						object: { kind: "media_server", body: server },
					},
				});
				snapshot = outcome.snapshot;
			}
			setLayout(snapshot);
			if (!discovered.length)
				onError(
					"No running CITP Media Server answered discovery. You can still add one manually.",
				);
		} catch (reason) {
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	async function remove(entry: VersionedMediaObject) {
		setBusy(true);
		try {
			const outcome = await documentSession.applyMediaIntent({
				requestId: id(),
				expectedRevision: entry.revision,
				action: {
					type: "delete",
					kind: entry.object.kind,
					id: entry.object.body.id,
				},
			});
			setLayout(outcome.snapshot);
			setSelected(null);
		} catch (reason) {
			onError(reason);
		} finally {
			setBusy(false);
		}
	}

	function add() {
		if (tab === "servers") {
			setServerToLink(null);
			setAddServerRequest((request) => request + 1);
		} else if (tab === "modules") {
			const module: LedModuleType = {
				id: id(),
				name: "500 mm LED module",
				widthMetres: 0.5,
				heightMetres: 0.5,
				pixelPitchMillimetres: 3.9,
				horizontalGapMetres: 0.005,
				verticalGapMetres: 0.005,
				pixelWidth: 128,
				pixelHeight: 128,
			};
			void apply({ kind: "led_module_type", body: module }, 0);
		} else if (tab === "surfaces") {
			const surface: MediaSurface = {
				id: id(),
				name: "Media Surface",
				sourceId: null,
				sections: [],
			};
			void apply({ kind: "media_surface", body: surface }, 0);
		} else {
			const surface = layout.surfaces[0]?.object;
			if (!surface || surface.kind !== "media_surface") {
				onError("Create a Media Surface before adding a projector.");
				return;
			}
			const projector: MediaProjector = {
				id: id(),
				name: "Projector",
				surfaceId: surface.body.id,
				transform: transform(),
				bodyModel: "projector",
				throwRatio: 1.5,
				lensShift: [0, 0],
				coneLengthMetres: 12,
				spill: 0.25,
			};
			void apply({ kind: "media_projector", body: projector }, 0);
		}
	}

	async function registerPatchedServers(
		fixtures: readonly { fixtureId: string; name: string }[],
	) {
		setBusy(true);
		try {
			let snapshot = layout;
			for (const [index, fixture] of fixtures.entries()) {
				const linkedEntry =
					index === 0 && serverToLink
						? snapshot.servers.find(
								(entry) => entry.object.body.id === serverToLink,
							)
						: undefined;
				if (linkedEntry?.object.kind === "media_server") {
					const outcome = await documentSession.applyMediaIntent({
						requestId: id(),
						expectedRevision: linkedEntry.revision,
						action: {
							type: "put",
							object: {
								...linkedEntry.object,
								body: {
									...linkedEntry.object.body,
									fixtureId: fixture.fixtureId,
								},
							},
						},
					});
					snapshot = outcome.snapshot;
					setSelected(linkedEntry.object.body.id);
					continue;
				}
				const server: MediaServer = {
					id: id(),
					name: fixture.name,
					fixtureId: fixture.fixtureId,
					citp: { host: "127.0.0.1", port: 4809 },
				};
				const outcome = await documentSession.applyMediaIntent({
					requestId: id(),
					expectedRevision: 0,
					action: {
						type: "put",
						object: { kind: "media_server", body: server },
					},
				});
				snapshot = outcome.snapshot;
				setSelected(server.id);
			}
			setLayout(snapshot);
		} catch (reason) {
			onError(reason);
		} finally {
			setServerToLink(null);
			setBusy(false);
		}
	}

	function patchCurrentServer() {
		if (current?.object.kind !== "media_server") return;
		setServerToLink(current.object.body.id);
		setAddServerRequest((request) => request + 1);
	}

	function addSource() {
		const server = layout.servers[0]?.object;
		if (!server || server.kind !== "media_server") return;
		void apply(
			{ kind: "media_source", body: newAdvertisedSource(server.body) },
			0,
		);
	}
	const addLabel = {
		servers: "Add server",
		surfaces: "Add surface",
		modules: "Add LED module",
		projectors: "Add projector",
	}[tab];

	return (
		<section className="viz-media-workspace">
			<WindowHeader
				title="Media"
				dragHandleProps={{
					"data-tauri-drag-region": true,
					onPointerDown: beginWindowDrag,
				}}
				groups={[
					{
						id: "media-actions",
						actions: [
							...(tab === "servers"
								? [
										{
											id: "discover",
											label: "Discover servers",
											disabled: busy,
											onPress: () => void discoverServers(),
										},
										{
											id: "enumerate",
											label: "Enumerate outputs",
											disabled: busy || !layout.servers.length,
											onPress: () => void enumerateOutputs(),
										},
										{
											id: "manual-output",
											label: "Add output manually",
											disabled: busy || !layout.servers.length,
											onPress: addSource,
										},
									]
								: []),
							{
								id: "add",
								label: addLabel,
								disabled: busy,
								onPress: add,
							},
						],
					},
					{
						id: "media-pages",
						kind: "tabs",
						activeId: tab,
						onActiveChange: (id) => {
							setTab(id as WorkspaceTab);
							setSelected(null);
						},
						actions: [
							{ id: "servers", label: "Servers" },
							{ id: "surfaces", label: "Surfaces" },
							{ id: "modules", label: "LED module types" },
							{ id: "projectors", label: "Projectors" },
						],
					},
				]}
			/>
			{tab === "servers" ? (
				<ServerPatch
					layout={layout}
					current={current}
					busy={busy}
					onSelect={setSelected}
					onLayoutChange={setLayout}
					onSave={apply}
					onDelete={remove}
					linkedFixture={linkedFixture}
					onOpenAddress={() => setAddressRequest((request) => request + 1)}
					onPatchServer={patchCurrentServer}
				/>
			) : entries.length === 0 ? (
				<MediaEmptyState tab={tab} />
			) : (
				<div className="viz-media-columns">
					<aside className="viz-media-list">
						{entries.map((entry) => (
							<Button
								key={`${entry.object.kind}-${entry.object.body.id}`}
								active={entry === current}
								onClick={() => setSelected(entry.object.body.id)}
							>
								{label(entry)}
								<small>{entry.object.kind.replaceAll("_", " ")}</small>
							</Button>
						))}
					</aside>
					<main className="viz-media-editor">
						{current ? (
							<ObjectEditor
								entry={current}
								layout={layout}
								onLayoutChange={setLayout}
								disabled={busy}
								onSave={apply}
								onDelete={() => remove(current)}
							/>
						) : null}
					</main>
				</div>
			)}
			<FixtureAddFlow
				scope="media"
				addRequest={addServerRequest}
				initialTypeFilter="media_server"
				onFixturesAdded={registerPatchedServers}
			/>
			<FixtureAddressFlow
				fixtureId={linkedFixtureId}
				openRequest={addressRequest}
			/>
		</section>
	);
}

function MediaEmptyState({ tab }: { tab: Exclude<WorkspaceTab, "servers"> }) {
	const copy = {
		surfaces: {
			title: "No media surfaces",
			body: "Media Surfaces are the physical projection screens, TVs, and LED walls that display a selected server output or fallback image.",
		},
		modules: {
			title: "No LED module types",
			body: "LED module types define panel size, pixel pitch, resolution, and gaps so reusable modules can build accurate LED walls.",
		},
		projectors: {
			title: "No projectors",
			body: "Projectors place a body, lens, throw, and light cone in the venue and link their brightness and colour to a Media Surface.",
		},
	}[tab];
	return (
		<div className="viz-media-empty-state">
			<h2>{copy.title}</h2>
			<p>{copy.body}</p>
		</div>
	);
}

function ServerPatch({
	layout,
	current,
	busy,
	onSelect,
	onLayoutChange,
	onSave,
	onDelete,
	linkedFixture,
	onOpenAddress,
	onPatchServer,
}: {
	layout: MediaLayoutSnapshot;
	current?: VersionedMediaObject;
	busy: boolean;
	onSelect: (id: string) => void;
	onLayoutChange: (layout: MediaLayoutSnapshot) => void;
	onSave: (object: MediaObject, revision: number) => Promise<void>;
	onDelete: (entry: VersionedMediaObject) => Promise<void>;
	linkedFixture: ReturnType<typeof usePatchFixture>;
	onOpenAddress: () => void;
	onPatchServer: () => void;
}) {
	return (
		<div className="viz-media-server-patch">
			{layout.servers.length ? (
				<div className="viz-media-server-table-wrap">
					<table className="viz-media-server-table">
						<thead>
							<tr>
								<th>Server</th>
								<th>CITP endpoint</th>
								<th>Outputs</th>
								<th>Last known endpoint</th>
							</tr>
						</thead>
						<tbody>
							{layout.servers.map((entry) => {
								if (entry.object.kind !== "media_server") return null;
								const server = entry.object.body;
								return (
									<tr
										key={server.id}
										className={entry === current ? "selected" : ""}
										onClick={() => onSelect(server.id)}
									>
										<td>{server.name}</td>
										<td>
											{server.citp.host}:{server.citp.port}
										</td>
										<td>
											{
												layout.sources.filter(
													(source) =>
														source.object.kind === "media_source" &&
														source.object.body.serverId === server.id,
												).length
											}
										</td>
										<td>{server.lastKnownEndpoint ?? "—"}</td>
									</tr>
								);
							})}
						</tbody>
					</table>
				</div>
			) : (
				<div className="viz-media-empty-state">
					<h2>No media servers patched</h2>
					<p>
						Media Servers are patched DMX fixtures that control layers and
						outputs. CITP discovery then finds their live preview sources
						independently.
					</p>
				</div>
			)}
			<div className="viz-media-server-editor">
				{current ? (
					<ObjectEditor
						entry={current}
						layout={layout}
						onLayoutChange={onLayoutChange}
						disabled={busy}
						onSave={onSave}
						onDelete={() => void onDelete(current)}
						linkedFixture={linkedFixture}
						onOpenAddress={onOpenAddress}
						onPatchServer={onPatchServer}
					/>
				) : null}
			</div>
		</div>
	);
}

function ObjectEditor({
	entry,
	layout,
	onLayoutChange,
	disabled,
	onSave,
	onDelete,
	linkedFixture,
	onOpenAddress,
	onPatchServer,
}: {
	entry: VersionedMediaObject;
	layout: MediaLayoutSnapshot;
	onLayoutChange: (layout: MediaLayoutSnapshot) => void;
	disabled: boolean;
	onSave: (object: MediaObject, revision: number) => Promise<void>;
	onDelete: () => void;
	linkedFixture?: ReturnType<typeof usePatchFixture>;
	onOpenAddress?: () => void;
	onPatchServer?: () => void;
}) {
	const [object, setObject] = useState(entry.object);
	useEffect(() => setObject(entry.object), [entry]);
	const update = <T extends MediaObject>(next: T["body"]) =>
		setObject({ ...object, body: next } as MediaObject);
	return (
		<form
			className="viz-media-object-editor"
			onSubmit={(event) => {
				event.preventDefault();
				void onSave(object, entry.revision);
			}}
		>
			<header>
				<h2>{label(entry)}</h2>
				<div>
					<Button type="button" onClick={onDelete} disabled={disabled}>
						Delete
					</Button>
					<Button type="submit" disabled={disabled}>
						Save
					</Button>
				</div>
			</header>
			{object.kind === "media_server" ? (
				<>
					{object.body.fixtureId ? (
						<DmxAddressField
							label="DMX patch"
							value={
								linkedFixture?.universe && linkedFixture.address
									? `${linkedFixture.universe}.${linkedFixture.address}`
									: ""
							}
							details={linkedFixture?.definition.mode ?? "Linked fixture"}
							disabled={!linkedFixture || disabled}
							onOpen={() => onOpenAddress?.()}
						/>
					) : (
						<div className="viz-media-unlinked-patch">
							<span>DMX patch</span>
							<Button type="button" disabled={disabled} onClick={onPatchServer}>
								Patch media server
							</Button>
						</div>
					)}
					<ServerEditor value={object.body} onChange={update} />
				</>
			) : null}
			{object.kind === "media_source" ? (
				<SourceEditor value={object.body} onChange={update} />
			) : null}
			{object.kind === "led_module_type" ? (
				<ModuleEditor value={object.body} onChange={update} />
			) : null}
			{object.kind === "media_surface" ? (
				<SurfaceEditor
					value={object.body}
					layout={layout}
					onLayoutChange={onLayoutChange}
					onChange={update}
				/>
			) : null}
			{object.kind === "media_projector" ? (
				<ProjectorEditor
					value={object.body}
					surfaces={layout.surfaces}
					onChange={update}
				/>
			) : null}
			{object.kind === "media_fallback_asset" ? (
				<p>
					Immutable fallback image · {object.body.width} × {object.body.height}
				</p>
			) : null}
		</form>
	);
}

function Field({
	label,
	children,
}: {
	label: string;
	children: React.ReactNode;
}) {
	return (
		// biome-ignore lint/a11y/noLabelWithoutControl: Every caller supplies its input control as the nested child.
		<label className="viz-media-field">
			<span>{label}</span>
			{children}
		</label>
	);
}

function ServerEditor({
	value,
	onChange,
}: {
	value: MediaServer;
	onChange: (value: MediaServer) => void;
}) {
	return (
		<div className="viz-media-form">
			<Field label="Name">
				<input
					value={value.name}
					onChange={(event) => onChange({ ...value, name: event.target.value })}
				/>
			</Field>
			<Field label="CITP host">
				<input
					value={value.citp.host}
					onChange={(event) =>
						onChange({
							...value,
							citp: { ...value.citp, host: event.target.value },
						})
					}
				/>
			</Field>
			<Field label="CITP port">
				<NumberInput
					value={value.citp.port}
					min={1}
					max={65535}
					step={1}
					onChange={(port) =>
						onChange({ ...value, citp: { ...value.citp, port } })
					}
				/>
			</Field>
			<Field label="Discovery identity">
				<input
					value={value.citp.discoveryIdentity ?? ""}
					onChange={(event) =>
						onChange({
							...value,
							citp: {
								...value.citp,
								discoveryIdentity: event.target.value || null,
							},
						})
					}
				/>
			</Field>
		</div>
	);
}

function SourceEditor({
	value,
	onChange,
}: {
	value: MediaSource;
	onChange: (value: MediaSource) => void;
}) {
	return (
		<div className="viz-media-form">
			<Field label="Name">
				<input
					value={value.name}
					onChange={(event) => onChange({ ...value, name: event.target.value })}
				/>
			</Field>
			<Field label="Advertised source ID">
				<NumberInput
					value={value.advertisedSourceId}
					min={0}
					max={65535}
					step={1}
					onChange={(advertisedSourceId) =>
						onChange({ ...value, advertisedSourceId })
					}
				/>
			</Field>
			<Field label="Output name">
				<input
					value={value.outputName ?? ""}
					onChange={(event) =>
						onChange({ ...value, outputName: event.target.value || null })
					}
				/>
			</Field>
		</div>
	);
}

function ModuleEditor({
	value,
	onChange,
}: {
	value: LedModuleType;
	onChange: (value: LedModuleType) => void;
}) {
	return (
		<div className="viz-media-form">
			<Field label="Name">
				<input
					value={value.name}
					onChange={(event) => onChange({ ...value, name: event.target.value })}
				/>
			</Field>
			{(
				[
					"widthMetres",
					"heightMetres",
					"pixelPitchMillimetres",
					"horizontalGapMetres",
					"verticalGapMetres",
					"pixelWidth",
					"pixelHeight",
				] as const
			).map((key) => (
				<Field
					key={key}
					label={key.replace(/[A-Z]/g, (match) => ` ${match.toLowerCase()}`)}
				>
					<NumberInput
						value={value[key]}
						min={0}
						step={
							key.startsWith("pixel") && key !== "pixelPitchMillimetres"
								? 1
								: 0.001
						}
						onChange={(next) => onChange({ ...value, [key]: next })}
					/>
				</Field>
			))}
		</div>
	);
}

function SurfaceEditor({
	value,
	layout,
	onLayoutChange,
	onChange,
}: {
	value: MediaSurface;
	layout: MediaLayoutSnapshot;
	onLayoutChange: (layout: MediaLayoutSnapshot) => void;
	onChange: (value: MediaSurface) => void;
}) {
	async function importFallback() {
		const path = await open({
			multiple: false,
			filters: [
				{ name: "Fallback image", extensions: ["png", "jpg", "jpeg", "webp"] },
			],
		});
		if (!path) return;
		const imported = await documentSession.importMediaFallback(path);
		onLayoutChange(imported.outcome.snapshot);
		onChange({ ...value, fallback: imported.reference });
	}
	function addSection(type: "projection_screen" | "tv" | "led") {
		const common = {
			id: id(),
			name: "Section",
			transform: transform(),
			widthMetres: 4,
			heightMetres: 2.25,
			crop: { left: 0, top: 0, width: 1, height: 1 },
		};
		let section: MediaSurfaceSection;
		if (type === "projection_screen")
			section = {
				...common,
				type,
				material: { type: "white" },
				edge_feather: 0.02,
			};
		else if (type === "tv")
			section = { ...common, type, bezel_metres: 0.025, spill: 0.2 };
		else {
			const module = layout.ledModuleTypes[0]?.object;
			if (!module || module.kind !== "led_module_type") return;
			section = {
				...common,
				type,
				module_type_id: module.body.id,
				rows: 4,
				columns: 8,
				occupied_cells: Array.from({ length: 32 }, (_, index) => index),
			};
		}
		onChange({ ...value, sections: [...value.sections, section] });
	}
	return (
		<div className="viz-media-form viz-media-surface-form">
			<Field label="Name">
				<input
					value={value.name}
					onChange={(event) => onChange({ ...value, name: event.target.value })}
				/>
			</Field>
			<Field label="Source">
				<select
					value={value.sourceId ?? ""}
					onChange={(event) =>
						onChange({ ...value, sourceId: event.target.value || null })
					}
				>
					<option value="">Fallback only / black</option>
					{layout.sources.map((entry) => (
						<option key={entry.object.body.id} value={entry.object.body.id}>
							{label(entry)}
						</option>
					))}
				</select>
			</Field>
			<Field label="Fallback">
				<span>
					{value.fallback
						? `${value.fallback.width} × ${value.fallback.height}`
						: "None"}{" "}
					<Button type="button" onClick={() => void importFallback()}>
						Import image
					</Button>
				</span>
			</Field>
			<div className="viz-media-section-actions">
				<Button type="button" onClick={() => addSection("projection_screen")}>
					Add screen
				</Button>
				<Button type="button" onClick={() => addSection("tv")}>
					Add TV
				</Button>
				<Button
					type="button"
					disabled={!layout.ledModuleTypes.length}
					onClick={() => addSection("led")}
				>
					Add LED wall
				</Button>
			</div>
			{value.sections.map((section, index) => (
				<SectionEditor
					key={section.id}
					value={section}
					onChange={(next) =>
						onChange({
							...value,
							sections: value.sections.map((item, itemIndex) =>
								itemIndex === index ? next : item,
							),
						})
					}
					onDelete={() =>
						onChange({
							...value,
							sections: value.sections.filter((item) => item.id !== section.id),
						})
					}
				/>
			))}
		</div>
	);
}

function SectionEditor({
	value,
	onChange,
	onDelete,
}: {
	value: MediaSurfaceSection;
	onChange: (value: MediaSurfaceSection) => void;
	onDelete: () => void;
}) {
	return (
		<fieldset className="viz-media-section">
			<legend>
				{value.name} · {value.type.replaceAll("_", " ")}
			</legend>
			<Button type="button" onClick={onDelete}>
				Remove
			</Button>
			<Field label="Name">
				<input
					value={value.name}
					onChange={(event) => onChange({ ...value, name: event.target.value })}
				/>
			</Field>
			<TransformEditor
				value={value.transform}
				onChange={(transform) => onChange({ ...value, transform })}
			/>
			<Field label="Width (m)">
				<NumberInput
					min={0.01}
					value={value.widthMetres}
					onChange={(widthMetres) => onChange({ ...value, widthMetres })}
				/>
			</Field>
			<Field label="Height (m)">
				<NumberInput
					min={0.01}
					value={value.heightMetres}
					onChange={(heightMetres) => onChange({ ...value, heightMetres })}
				/>
			</Field>
			<fieldset className="viz-media-crop">
				<legend className="sr-only">Normalized top-left crop</legend>
				<div
					className="viz-media-crop-preview"
					role="img"
					aria-label="Crop preview"
				>
					<div
						style={{
							left: `${value.crop.left * 100}%`,
							top: `${value.crop.top * 100}%`,
							width: `${value.crop.width * 100}%`,
							height: `${value.crop.height * 100}%`,
						}}
					/>
				</div>
				{(["left", "top", "width", "height"] as const).map((key) => (
					<Field key={key} label={`Crop ${key}`}>
						<NumberInput
							value={value.crop[key]}
							min={0}
							max={1}
							onChange={(next) =>
								onChange({ ...value, crop: { ...value.crop, [key]: next } })
							}
						/>
					</Field>
				))}
			</fieldset>
			{value.type === "led" ? (
				<LedGrid section={value} onChange={onChange} />
			) : null}
		</fieldset>
	);
}

function LedGrid({
	section,
	onChange,
}: {
	section: Extract<MediaSurfaceSection, { type: "led" }>;
	onChange: (value: MediaSurfaceSection) => void;
}) {
	const occupied = new Set(section.occupied_cells);
	return (
		<div
			className="viz-led-grid"
			style={{ gridTemplateColumns: `repeat(${section.columns}, 24px)` }}
		>
			{Array.from({ length: section.rows * section.columns }, (_, index) => (
				<button
					key={index}
					type="button"
					className={occupied.has(index) ? "occupied" : ""}
					aria-label={`LED cell ${index + 1}`}
					onClick={() => {
						occupied.has(index) ? occupied.delete(index) : occupied.add(index);
						onChange({
							...section,
							occupied_cells: [...occupied].sort((a, b) => a - b),
						});
					}}
				/>
			))}
		</div>
	);
}

function ProjectorEditor({
	value,
	surfaces,
	onChange,
}: {
	value: MediaProjector;
	surfaces: VersionedMediaObject[];
	onChange: (value: MediaProjector) => void;
}) {
	return (
		<div className="viz-media-form">
			<Field label="Name">
				<input
					value={value.name}
					onChange={(event) => onChange({ ...value, name: event.target.value })}
				/>
			</Field>
			<TransformEditor
				value={value.transform}
				onChange={(transform) => onChange({ ...value, transform })}
			/>
			<Field label="Linked Media Surface">
				<select
					value={value.surfaceId}
					onChange={(event) =>
						onChange({ ...value, surfaceId: event.target.value })
					}
				>
					{surfaces.map((entry) => (
						<option key={entry.object.body.id} value={entry.object.body.id}>
							{label(entry)}
						</option>
					))}
				</select>
			</Field>
			<Field label="Body model">
				<input
					value={value.bodyModel}
					onChange={(event) =>
						onChange({ ...value, bodyModel: event.target.value })
					}
				/>
			</Field>
			<Field label="Throw ratio">
				<NumberInput
					min={0.1}
					value={value.throwRatio}
					onChange={(throwRatio) => onChange({ ...value, throwRatio })}
				/>
			</Field>
			<Field label="Horizontal lens shift">
				<NumberInput
					min={-1}
					max={1}
					value={value.lensShift[0]}
					onChange={(shift) =>
						onChange({ ...value, lensShift: [shift, value.lensShift[1]] })
					}
				/>
			</Field>
			<Field label="Vertical lens shift">
				<NumberInput
					min={-1}
					max={1}
					value={value.lensShift[1]}
					onChange={(shift) =>
						onChange({ ...value, lensShift: [value.lensShift[0], shift] })
					}
				/>
			</Field>
			<Field label="Cone length (m)">
				<NumberInput
					min={0.1}
					value={value.coneLengthMetres}
					onChange={(coneLengthMetres) =>
						onChange({ ...value, coneLengthMetres })
					}
				/>
			</Field>
			<Field label="Spill">
				<NumberInput
					min={0}
					max={1}
					value={value.spill}
					onChange={(spill) => onChange({ ...value, spill })}
				/>
			</Field>
		</div>
	);
}

function TransformEditor({
	value,
	onChange,
}: {
	value: MediaSurfaceSection["transform"];
	onChange: (value: MediaSurfaceSection["transform"]) => void;
}) {
	return (
		<fieldset className="viz-media-transform">
			<legend>3D transform</legend>
			{(["X", "Y", "Z"] as const).map((axis, index) => (
				<Field key={`position-${axis}`} label={`${axis} (m)`}>
					<NumberInput
						value={value.positionMetres[index]}
						onChange={(next) => {
							const positionMetres = [...value.positionMetres] as [
								number,
								number,
								number,
							];
							positionMetres[index] = next;
							onChange({ ...value, positionMetres });
						}}
					/>
				</Field>
			))}
			{(["X", "Y", "Z"] as const).map((axis, index) => (
				<Field key={`rotation-${axis}`} label={`${axis} rotation`}>
					<NumberInput
						step={1}
						value={value.rotationDegrees[index]}
						onChange={(next) => {
							const rotationDegrees = [...value.rotationDegrees] as [
								number,
								number,
								number,
							];
							rotationDegrees[index] = next;
							onChange({ ...value, rotationDegrees });
						}}
					/>
				</Field>
			))}
		</fieldset>
	);
}

export function newAdvertisedSource(server: MediaServer): MediaSource {
	return {
		id: id(),
		serverId: server.id,
		advertisedSourceId: 0,
		name: "Program",
		outputName: "Program",
		aspectRatio: 16 / 9,
	};
}
