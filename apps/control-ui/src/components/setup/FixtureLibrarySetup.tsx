import JSZip from "jszip";
import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";
import type { FixtureDefinition, FixtureProfile } from "../../api/types";
import { Button, ModalTitleBar, Select } from "../common";
import { SearchBar } from "../common/SearchBar";
import { RootConfinedFilePickerButton } from "../files/RootConfinedFilePickerButton";
import { WindowScrollArea } from "../window-kit";
import { FixtureProfileEditor } from "./FixtureProfileEditor";
import {
	blankFixtureProfile,
	fixtureDefinitionKey,
	fixtureProfileFromDefinition,
	fixtureProfileFromDefinitions,
	mergeFixtureDefinitions,
} from "./fixtureProfileModel";
import { compareFixtureManufacturers, groupFixtureFamilies } from "./patchUtils";

export const FIXTURE_TYPES = [
	"dimmer",
	"fogger",
	"profile",
	"wash",
	"wash mover",
	"spot mover",
	"beam mover",
	"strobe",
	"media server",
	"pixel fixture",
	"other",
];

export function blankDefinition(): FixtureDefinition {
	return {
		schema_version: 1,
		id: crypto.randomUUID(),
		revision: 1,
		manufacturer: "",
		device_type: "other",
		name: "",
		model: "",
		mode: "Standard",
		footprint: 1,
		heads: [
			{
				index: 0,
				name: "Main",
				shared: true,
				parameters: [
					{
						attribute: "intensity",
						components: [{ offset: 0, byte_order: "msb_first" }],
						default: 0,
						virtual_dimmer: false,
						capabilities: [],
					},
				],
			},
		],
		color_calibration: null,
		physical: {},
		model_asset: null,
		icon_asset: null,
		hazardous: false,
		direct_control_protocols: [],
		signal_loss_policy: { type: "hold_last" },
		safe_values: {},
	};
}

const attrName = (value: string) => {
	const normalized = value
		.replace(/([a-z])([A-Z])/g, "$1.$2")
		.replace(/\s+/g, ".")
		.toLowerCase();
	const aliases: Record<string, string> = {
		dimmer: "intensity",
		"color.add_r": "color.red",
		"color.add_g": "color.green",
		"color.add_b": "color.blue",
		"color.add_w": "color.white",
		"color.add_ww": "color.warm_white",
		"color.add_cw": "color.cold_white",
		"color.sub_c": "color.cyan",
		"color.sub_m": "color.magenta",
		"color.sub_y": "color.yellow",
	};
	const wheel = normalized.match(/^color(?:\.wheel)?_?(\d+)$/);
	return (
		aliases[normalized] ?? (wheel ? `color.wheel.${wheel[1]}` : normalized)
	);
};

type HeadDraft = { name: string; master: boolean; channels: string };

function parseDmx(value: string | null | undefined, fallback: number) {
	const parsed = Number((value ?? "").split("/")[0]);
	return Number.isFinite(parsed)
		? Math.max(0, Math.min(255, parsed))
		: fallback;
}

function parseDefault(value: string | null | undefined, bytes: number) {
	const parsed = Number((value ?? "").split("/")[0]);
	return Number.isFinite(parsed)
		? Math.max(0, Math.min(1, parsed / (2 ** (bytes * 8) - 1)))
		: 0;
}

export function parseHeadDrafts(heads: HeadDraft[]) {
	let offset = 0;
	const splitChannels = (value: string) => {
		const result: string[] = [];
		let depth = 0;
		let start = 0;
		for (let index = 0; index < value.length; index++) {
			if (value[index] === "{" || value[index] === "[") depth++;
			if (value[index] === "}" || value[index] === "]") depth--;
			if (value[index] === "," && depth === 0) {
				result.push(value.slice(start, index));
				start = index + 1;
			}
		}
		result.push(value.slice(start));
		return result;
	};
	const parsedHeads = heads.map((head, headIndex) => ({
		index: headIndex,
		name: head.name.trim() || `Head ${headIndex + 1}`,
		shared: head.master,
		parameters: splitChannels(head.channels)
			.map((raw) => raw.trim())
			.filter(Boolean)
			.map((raw) => {
				const capabilitiesText = raw.match(/\{(.+)\}/)?.[1] ?? "";
				const rangeText = raw.match(/\[(-?[\d.]+),(-?[\d.]+)(?:,([^\]]+))?\]/);
				const clean = raw.replace(/\{.+\}/, "").replace(/\[.+\]/, "");
				const [attribute, resolution] = clean.split(":");
				const bytes = resolution === "16" ? 2 : 1;
				const start = offset;
				offset += bytes;
				return {
					attribute: attrName(attribute),
					components: Array.from({ length: bytes }, (_, component) => ({
						offset: start + component,
						byte_order: "msb_first" as const,
					})),
					default: attribute.toLowerCase().includes("shutter") ? 1 : 0,
					virtual_dimmer: false,
					metadata: {
						physical_min: rangeText ? Number(rangeText[1]) : 0,
						physical_max: rangeText ? Number(rangeText[2]) : 1,
						unit: rangeText?.[3] ?? null,
						invert: false,
						wrap: attribute.toLowerCase().includes("pan"),
						curve: "linear",
					},
					capabilities: capabilitiesText
						.split("|")
						.filter(Boolean)
						.map((entry) => {
							const [name, range = "0-255"] = entry.split("=");
							const [from, to = from] = range.split("-").map(Number);
							return {
								name: name.trim(),
								dmx_from: from,
								dmx_to: to,
								preset_family: attribute.toLowerCase().includes("gobo")
									? "gobo"
									: null,
							};
						}),
				};
			}),
	}));
	return { heads: parsedHeads, footprint: offset };
}

export async function importGdtfData(
	data: ArrayBuffer | Uint8Array,
	fileName: string,
): Promise<FixtureDefinition[]> {
	const zip = await JSZip.loadAsync(data);
	const description = zip.file(/(^|\/)description\.xml$/i)[0];
	if (!description) throw new Error("The GDTF archive has no description.xml");
	const xml = new DOMParser().parseFromString(
		await description.async("text"),
		"application/xml",
	);
	const error = xml.querySelector("parsererror");
	if (error) throw new Error("The GDTF description.xml is invalid");
	const root = xml.querySelector("FixtureType");
	if (!root) throw new Error("The GDTF file contains no FixtureType");
	const manufacturer = root.getAttribute("Manufacturer") || "Unknown";
	const model =
		root.getAttribute("ShortName") ||
		root.getAttribute("Name") ||
		fileName.replace(/\.gdtf$/i, "");
	const modelEntry = Object.values(zip.files).find(
		(entry) => !entry.dir && entry.name.toLowerCase().endsWith(".glb"),
	);
	const modelAsset = modelEntry
		? `data:model/gltf-binary;base64,${await modelEntry.async("base64")}`
		: null;
	const emitters = [
		...root.querySelectorAll("PhysicalDescriptions > Emitters > Emitter"),
	]
		.map((emitter) => {
			const [x = 0, y = 0, luminance = 1] = (
				emitter.getAttribute("Color") ?? ""
			)
				.split(",")
				.map(Number);
			return {
				name: emitter.getAttribute("Name") || `Emitter ${x},${y}`,
				xyz: {
					x: y > 0 ? (x * luminance) / y : 0,
					y: luminance,
					z: y > 0 ? ((1 - x - y) * luminance) / y : 0,
				},
				limit: 1,
			};
		})
		.filter(
			(emitter) =>
				emitter.xyz.x >= 0 && emitter.xyz.y >= 0 && emitter.xyz.z >= 0,
		);
	const colorCalibration =
		emitters.length >= 3
			? {
					emitters,
					correction_matrix: [
						[1, 0, 0],
						[0, 1, 0],
						[0, 0, 1],
					],
				}
			: null;
	const modes = [...xml.querySelectorAll("DMXModes > DMXMode")];
	return (modes.length ? modes : [root]).map((mode, modeIndex) => {
		const channels = [...mode.querySelectorAll("DMXChannels > DMXChannel")];
		const headNames = [
			...new Set(
				channels.map((channel) => channel.getAttribute("Geometry") || "Main"),
			),
		];
		let footprint = 1;
		const heads = headNames.map((headName, headIndex) => ({
			index: headIndex,
			name: headName,
			shared: headNames.length === 1 || /master|base/i.test(headName),
			parameters: channels
				.filter(
					(channel) =>
						(channel.getAttribute("Geometry") || "Main") === headName,
				)
				.map((channel) => {
					const offsets = (channel.getAttribute("Offset") || "1")
						.split(",")
						.map(Number)
						.filter(Number.isFinite)
						.map((value) => Math.max(0, value - 1));
					footprint = Math.max(footprint, ...offsets.map((value) => value + 1));
					const logical = channel.querySelector("LogicalChannel");
					const fn = logical?.querySelector("ChannelFunction");
					const attribute = attrName(
						logical?.getAttribute("Attribute") ||
							fn?.getAttribute("Attribute") ||
							channel.getAttribute("Name") ||
							`channel.${offsets[0] + 1}`,
					);
					const physicalFrom = Number(fn?.getAttribute("PhysicalFrom") ?? 0);
					const physicalTo = Number(fn?.getAttribute("PhysicalTo") ?? 1);
					return {
						attribute,
						components: offsets.map((offset) => ({
							offset,
							byte_order: "msb_first" as const,
						})),
						default: parseDefault(
							channel.getAttribute("Default") || fn?.getAttribute("Default"),
							offsets.length,
						),
						virtual_dimmer: false,
						metadata: {
							physical_min: physicalFrom,
							physical_max:
								physicalTo === physicalFrom ? physicalFrom + 1 : physicalTo,
							unit: null,
							invert: false,
							wrap: attribute.includes("pan"),
							curve: "linear",
						},
						capabilities: [...channel.querySelectorAll("ChannelSet")].map(
							(set, index, all) => ({
								name: set.getAttribute("Name") || attribute,
								dmx_from: parseDmx(
									set.getAttribute("DMXFrom"),
									Math.round((index * 256) / all.length),
								),
								dmx_to: parseDmx(
									set.getAttribute("DMXTo"),
									Math.round(((index + 1) * 256) / all.length) - 1,
								),
								preset_family: attribute.includes("gobo") ? "gobo" : null,
							}),
						),
					};
				}),
		}));
		const allAttributes = heads
			.flatMap((head) =>
				head.parameters.map((parameter) => parameter.attribute),
			)
			.join(" ");
		const classify = /fog|haze/.test(`${model} ${allAttributes}`.toLowerCase())
			? "fogger"
			: /media/.test(model.toLowerCase())
				? "media server"
				: /pan|tilt/.test(allAttributes) &&
						/wash|color/.test(`${model} ${allAttributes}`.toLowerCase())
					? "wash mover"
					: /pan|tilt/.test(allAttributes)
						? "spot mover"
						: /wash|color/.test(`${model} ${allAttributes}`.toLowerCase())
							? "wash"
							: /shutter|gobo|focus/.test(allAttributes)
								? "profile"
								: "other";
		const pan = heads
			.flatMap((head) => head.parameters)
			.find((parameter) => parameter.attribute.includes("pan"))?.metadata;
		const tilt = heads
			.flatMap((head) => head.parameters)
			.find((parameter) => parameter.attribute.includes("tilt"))?.metadata;
		return {
			...blankDefinition(),
			id: crypto.randomUUID(),
			manufacturer,
			device_type: classify,
			name: model,
			model,
			model_asset: modelAsset,
			color_calibration: colorCalibration,
			mode: mode.getAttribute("Name") || `Mode ${modeIndex + 1}`,
			footprint,
			heads,
			physical: {
				pan_range_degrees: pan
					? Math.abs(pan.physical_max - pan.physical_min)
					: null,
				tilt_range_degrees: tilt
					? Math.abs(tilt.physical_max - tilt.physical_min)
					: null,
			},
		};
	});
}

export async function importGdtf(file: File) {
	return importGdtfData(await file.arrayBuffer(), file.name);
}

export function FixtureLibrarySetup() {
	const server = useServer();
	const [busy, setBusy] = useState(false);
	const [selectedFamilyKey, setSelectedFamilyKey] = useState("");
	const [selectedModeKey, setSelectedModeKey] = useState("");
	const [modal, setModal] = useState<"gdtf" | "package" | null>(null);
	const [profileEditor, setProfileEditor] = useState<{
		draft: FixtureProfile;
		expectedRevision: number;
	} | null>(null);
	const [revisionHistory, setRevisionHistory] = useState<
		FixtureProfile[] | null
	>(null);
	const [revisionHistoryError, setRevisionHistoryError] = useState("");
	const [query, setQuery] = useState("");
	const [typeFilter, setTypeFilter] = useState("");
	const [manufacturer, setManufacturer] = useState("");
	const availableDefinitions = useMemo(
		() =>
			mergeFixtureDefinitions(server.fixtureProfiles, server.fixtureLibrary),
		[server.fixtureProfiles, server.fixtureLibrary],
	);
	const fixtureTypes = useMemo(
		() =>
			[
				...new Set(
					availableDefinitions.map((item) => item.device_type || "other"),
				),
			].sort(),
		[availableDefinitions],
	);
	const manufacturers = useMemo(
		() =>
			[...new Set(availableDefinitions.map((item) => item.manufacturer))]
				.filter(Boolean)
				.sort(compareFixtureManufacturers),
		[availableDefinitions],
	);
	const libraryFamilies = useMemo(
		() =>
			groupFixtureFamilies(
				availableDefinitions.filter((item) => {
					const needle = query.toLowerCase().trim();
					return (
						(!manufacturer || item.manufacturer === manufacturer) &&
						(!typeFilter || item.device_type === typeFilter) &&
						(!needle ||
							`${item.manufacturer} ${item.name} ${item.model} ${item.mode} ${item.device_type}`
								.toLowerCase()
								.includes(needle))
					);
				}),
			),
		[availableDefinitions, query, manufacturer, typeFilter],
	);
	const selectedLibraryFamily =
		libraryFamilies.find((family) => family.key === selectedFamilyKey) ??
		libraryFamilies[0] ??
		null;
	const selectedMode =
		selectedLibraryFamily?.modes.find(
			(mode) => fixtureDefinitionKey(mode) === selectedModeKey,
		) ??
		selectedLibraryFamily?.modes[0] ??
		null;
	const importFile = async (file?: File) => {
		if (!file) return;
		setBusy(true);
		try {
			const source = new Uint8Array(await file.arrayBuffer());
			const imported = await importGdtfData(source, file.name);
			const profile = fixtureProfileFromDefinitions(imported);
			const saved = imported.length
				? await server.saveFixtureProfile(profile, 0)
				: null;
			if (
				saved &&
				(await server.saveFixtureProfileSourceGdtf(
					saved.id,
					saved.revision,
					source,
				))
			) {
				setSelectedFamilyKey(
					`${saved.manufacturer}\0${saved.short_name || saved.name}`,
				);
				setSelectedModeKey(
					`${saved.id}:${saved.revision}:${saved.modes[0]?.id ?? saved.id}`,
				);
				setModal(null);
			}
		} finally {
			setBusy(false);
		}
	};
	const importPackage = async (file?: File) => {
		if (!file) return;
		setBusy(true);
		try {
			const imported = await server.importFixturePackage(
				new Uint8Array(await file.arrayBuffer()),
			);
			setSelectedFamilyKey(
				`${imported.manufacturer}\0${imported.short_name || imported.name}`,
			);
			setSelectedModeKey(
				`${imported.id}:${imported.revision}:${imported.modes[0]?.id ?? imported.id}`,
			);
			setModal(null);
		} finally {
			setBusy(false);
		}
	};
	const exportSelectedPackage = async () => {
		if (!selectedMode) return;
		const id = selectedMode.profile_id ?? selectedMode.id;
		const blob = await server.exportFixturePackage(id, selectedMode.revision);
		const url = URL.createObjectURL(blob);
		const anchor = document.createElement("a");
		anchor.href = url;
		anchor.download = `${selectedMode.manufacturer}-${selectedMode.name || selectedMode.model}.toskfixture`
			.replace(/[^a-z0-9._-]+/gi, "-")
			.toLowerCase();
		anchor.click();
		URL.revokeObjectURL(url);
	};
	const openCreate = () =>
		setProfileEditor({ draft: blankFixtureProfile(), expectedRevision: 0 });
	const openRevisionHistory = async () => {
		if (!selectedMode) return;
		setRevisionHistoryError("");
		try {
			setRevisionHistory(
				await server.fixtureProfileRevisions(
					selectedMode.profile_id ?? selectedMode.id,
				),
			);
		} catch (reason) {
			setRevisionHistory([]);
			setRevisionHistoryError(
				reason instanceof Error ? reason.message : String(reason),
			);
		}
	};
	const deleteRevision = async (profile: FixtureProfile) => {
		if (
			!window.confirm(
				`Delete ${profile.manufacturer} ${profile.name} revision ${profile.revision}? Patched shows keep their embedded snapshot.`,
			)
		)
			return;
		await server.deleteFixtureProfile(profile.id, profile.revision);
		const remaining = await server
			.fixtureProfileRevisions(profile.id)
			.catch(() => []);
		setRevisionHistory(remaining);
		if (!remaining.length) setRevisionHistory(null);
	};
	const [searchTarget, setSearchTarget] = useState<HTMLElement | null>(null);
	const [actionsTarget, setActionsTarget] = useState<HTMLElement | null>(null);
	useEffect(() => {
		setSearchTarget(
			document.getElementById("setup-section-search") ??
				document.getElementById("setup-section-actions"),
		);
		setActionsTarget(document.getElementById("setup-section-actions"));
	}, []);
	return (
		<div className="fixture-library-setup">
			{searchTarget &&
				createPortal(
					<SearchBar
							value={query}
							onChange={setQuery}
							filters={[
								{ id: "type", label: "Fixture type", options: fixtureTypes },
							]}
							values={{ type: typeFilter }}
							onFilterChange={(_, value) => setTypeFilter(value)}
							placeholder="Search manufacturer, fixture, mode, or type"
						/>,
					searchTarget,
				)}
			{actionsTarget && createPortal(
				<div className="setup-section-action-group">
					<Button onClick={() => setModal("package")}>Import fixture</Button>
					<Button onClick={() => setModal("gdtf")}>Import GDTF</Button>
					<Button onClick={openCreate}>Create fixture</Button>
				</div>,
				actionsTarget,
			)}
			{Boolean(server.fixtureProfileWarnings.length) && (
				<section
					className="fixture-migration-warnings"
					role="alert"
					aria-label="Fixture library migration warnings"
				>
					<h3>Fixture library needs attention</h3>
					{server.fixtureProfileWarnings.map((warning) => (
						<p key={warning}>{warning}</p>
					))}
				</section>
			)}
			<div className="fixture-library-columns">
				<section>
					<h3>Manufacturer</h3>
					<WindowScrollArea className="fixture-library-column-scroll">
						<Button
							className={!manufacturer ? "active" : ""}
							onClick={() => setManufacturer("")}
						>
							<span>All manufacturers</span>
						</Button>
						{manufacturers.map((name) => (
							<Button
								className={manufacturer === name ? "active" : ""}
								key={name}
								onClick={() => setManufacturer(name)}
							>
								<span>{name}</span>
							</Button>
						))}
					</WindowScrollArea>
				</section>
				<section>
					<h3>Fixture</h3>
					<WindowScrollArea className="fixture-library-column-scroll">
						{libraryFamilies.map((family) => (
							<Button
								className={
									selectedLibraryFamily?.key === family.key ? "active" : ""
								}
								key={family.key}
								onClick={() => {
									setSelectedFamilyKey(family.key);
									setSelectedModeKey(fixtureDefinitionKey(family.modes[0]));
								}}
							>
								<span>{family.name}</span>
								<small>
									{family.deviceType} · {family.modes.length} modes
								</small>
							</Button>
						))}
					</WindowScrollArea>
				</section>
				<section className="fixture-library-detail">
					<WindowScrollArea className="fixture-library-column-scroll">
						{selectedLibraryFamily && selectedMode ? (
							<>
								<h3>
									{selectedLibraryFamily.manufacturer}{" "}
									{selectedLibraryFamily.name}
								</h3>
								<label htmlFor="fixture-library-mode">
									Mode
									<Select
										id="fixture-library-mode"
										value={fixtureDefinitionKey(selectedMode)}
										onChange={(event) => setSelectedModeKey(event.target.value)}
									>
										{selectedLibraryFamily.modes.map((mode) => (
											<option
												value={fixtureDefinitionKey(mode)}
												key={fixtureDefinitionKey(mode)}
											>
												{mode.mode} · {mode.footprint}ch
											</option>
										))}
									</Select>
								</label>
								<dl>
									<dt>Type</dt>
									<dd>{selectedMode.device_type}</dd>
									<dt>DMX footprint</dt>
									<dd>{selectedMode.footprint} channels</dd>
									<dt>Heads</dt>
									<dd>{selectedMode.heads.length}</dd>
									<dt>Revision</dt>
									<dd>{selectedMode.revision}</dd>
									<dt>Physical</dt>
									<dd>
										{selectedMode.physical.width_millimetres ?? "?"} ×{" "}
										{selectedMode.physical.height_millimetres ?? "?"} ×{" "}
										{selectedMode.physical.depth_millimetres ?? "?"} mm
									</dd>
								</dl>
								<Button
									onClick={() => {
										const draft = fixtureProfileFromDefinition(selectedMode);
										setProfileEditor({
											draft,
											expectedRevision: Math.max(
												draft.revision,
												...server.fixtureProfiles
													.filter((profile) => profile.id === draft.id)
													.map((profile) => profile.revision),
											),
										});
									}}
								>
									Edit fixture
								</Button>
								<Button onClick={() => void openRevisionHistory()}>
									Revision history
								</Button>
								<Button onClick={() => void exportSelectedPackage()}>
									Export fixture
								</Button>
							</>
						) : (
							<p>No fixture matches this search.</p>
						)}
					</WindowScrollArea>
				</section>
			</div>
			{modal === "gdtf" && (
				<div className="stacked-modal-layer">
					<section className="nested-modal gdtf-import-modal">
						<ModalTitleBar
							title="Import GDTF"
							closeLabel="Close Import GDTF"
							onClose={() => setModal(null)}
						/>
						<p>
							Select a GDTF archive. Every DMX mode will be imported into the
							desk-wide fixture library.
						</p>
						<RootConfinedFilePickerButton
							variant="primary"
							disabled={busy}
							label={busy ? "Importing…" : "Choose GDTF file"}
							allowedExtensions={["gdtf"]}
							onFiles={(files) => importFile(files[0])}
						/>
					</section>
				</div>
			)}
			{modal === "package" && (
				<div className="stacked-modal-layer">
					<section className="nested-modal fixture-package-import-modal">
						<ModalTitleBar
							title="Import fixture"
							closeLabel="Close Import fixture"
							onClose={() => setModal(null)}
						/>
						<p>
							Select a transferable .toskfixture package. Its modes, photograph,
							stage icon, and 3D model travel together.
						</p>
						<RootConfinedFilePickerButton
							variant="primary"
							disabled={busy}
							label={busy ? "Importing…" : "Choose fixture package"}
							allowedExtensions={["toskfixture"]}
							onFiles={(files) => importPackage(files[0])}
						/>
					</section>
				</div>
			)}
			{revisionHistory && (
				<div
					className="stacked-modal-layer"
					onPointerDown={(event) =>
						event.target === event.currentTarget && setRevisionHistory(null)
					}
				>
					<section
						className="nested-modal fixture-revision-history"
						role="dialog"
						aria-modal="true"
						aria-label="Fixture revision history"
					>
						<header>
							<h2>Fixture revision history</h2>
							<Button
								aria-label="Close Fixture revision history"
								onClick={() => setRevisionHistory(null)}
							>
								×
							</Button>
						</header>
						{revisionHistoryError && <p role="alert">{revisionHistoryError}</p>}
						{!revisionHistory.length && !revisionHistoryError && (
							<p>No retained revisions.</p>
						)}
						<div>
							{[...revisionHistory]
								.sort((left, right) => right.revision - left.revision)
								.map((profile) => (
									<article key={`${profile.id}:${profile.revision}`}>
										<span>
											<b>Revision {profile.revision}</b>
											<small>
												{profile.manufacturer} {profile.name} ·{" "}
												{profile.modes.length} mode
												{profile.modes.length === 1 ? "" : "s"}
											</small>
										</span>
										<Button
											onClick={() => {
												setProfileEditor({
													draft: structuredClone(profile),
													expectedRevision: Math.max(
														...revisionHistory.map(
															(revision) => revision.revision,
														),
													),
												});
												setRevisionHistory(null);
											}}
										>
											Edit as new revision
										</Button>
										<Button
											className="danger"
											onClick={() => void deleteRevision(profile)}
										>
											Delete revision
										</Button>
									</article>
								))}
						</div>
						<p>
							Deleting a library revision never changes fixtures already patched
							into a show because each patch embeds its own portable snapshot.
						</p>
					</section>
				</div>
			)}
			{profileEditor && (
				<FixtureProfileEditor
					initialProfile={profileEditor.draft}
					expectedRevision={profileEditor.expectedRevision}
					manufacturers={manufacturers}
					attributeRegistry={server.bootstrap?.attribute_registry ?? []}
					onSave={server.saveFixtureProfile}
					onClose={() => setProfileEditor(null)}
				/>
			)}
		</div>
	);
}
