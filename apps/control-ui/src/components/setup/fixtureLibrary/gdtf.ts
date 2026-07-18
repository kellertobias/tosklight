import JSZip from "jszip";
import type { FixtureDefinition } from "../../../api/types";
import { blankDefinition, fixtureAttributeName } from "./definitions";

type FixtureHead = FixtureDefinition["heads"][number];
type FixtureParameter = FixtureHead["parameters"][number];

interface GdtfArchive {
	root: Element;
	manufacturer: string;
	model: string;
	modelAsset: string | null;
}

interface FootprintTracker {
	value: number;
}

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

async function loadGdtfArchive(
	data: ArrayBuffer | Uint8Array,
	fileName: string,
): Promise<GdtfArchive> {
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
	return { root, manufacturer, model, modelAsset };
}

function colorCalibration(root: Element) {
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
	return emitters.length >= 3
		? {
				emitters,
				correction_matrix: [
					[1, 0, 0],
					[0, 1, 0],
					[0, 0, 1],
				],
			}
		: null;
}

function channelCapabilities(channel: Element, attribute: string) {
	return [...channel.querySelectorAll("ChannelSet")].map((set, index, all) => ({
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
	}));
}

function parseGdtfChannel(
	channel: Element,
	footprint: FootprintTracker,
): FixtureParameter {
	const offsets = (channel.getAttribute("Offset") || "1")
		.split(",")
		.map(Number)
		.filter(Number.isFinite)
		.map((value) => Math.max(0, value - 1));
	footprint.value = Math.max(
		footprint.value,
		...offsets.map((value) => value + 1),
	);
	const logical = channel.querySelector("LogicalChannel");
	const fn = logical?.querySelector("ChannelFunction");
	const attribute = fixtureAttributeName(
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
			physical_max: physicalTo === physicalFrom ? physicalFrom + 1 : physicalTo,
			unit: null,
			invert: false,
			wrap: attribute.includes("pan"),
			curve: "linear",
		},
		capabilities: channelCapabilities(channel, attribute),
	};
}

function parseGdtfHead(
	headName: string,
	headIndex: number,
	headNames: string[],
	channels: Element[],
	footprint: FootprintTracker,
): FixtureHead {
	return {
		index: headIndex,
		name: headName,
		shared: headNames.length === 1 || /master|base/i.test(headName),
		parameters: channels
			.filter(
				(channel) => (channel.getAttribute("Geometry") || "Main") === headName,
			)
			.map((channel) => parseGdtfChannel(channel, footprint)),
	};
}

function fixtureType(model: string, allAttributes: string) {
	const searchable = `${model} ${allAttributes}`.toLowerCase();
	if (/fog|haze/.test(searchable)) return "fogger";
	if (/media/.test(model.toLowerCase())) return "media server";
	if (/pan|tilt/.test(allAttributes) && /wash|color/.test(searchable)) {
		return "wash mover";
	}
	if (/pan|tilt/.test(allAttributes)) return "spot mover";
	if (/wash|color/.test(searchable)) return "wash";
	if (/shutter|gobo|focus/.test(allAttributes)) return "profile";
	return "other";
}

function physicalRange(
	heads: FixtureHead[],
	attribute: "pan" | "tilt",
): number | null {
	const metadata = heads
		.flatMap((head) => head.parameters)
		.find((parameter) => parameter.attribute.includes(attribute))?.metadata;
	return metadata
		? Math.abs(metadata.physical_max - metadata.physical_min)
		: null;
}

function modeDefinition(
	archive: GdtfArchive,
	mode: Element,
	modeIndex: number,
	calibration: ReturnType<typeof colorCalibration>,
): FixtureDefinition {
	const channels = [...mode.querySelectorAll("DMXChannels > DMXChannel")];
	const headNames = [
		...new Set(
			channels.map((channel) => channel.getAttribute("Geometry") || "Main"),
		),
	];
	const footprint = { value: 1 };
	const heads = headNames.map((headName, headIndex) =>
		parseGdtfHead(headName, headIndex, headNames, channels, footprint),
	);
	const allAttributes = heads
		.flatMap((head) => head.parameters.map((parameter) => parameter.attribute))
		.join(" ");
	return {
		...blankDefinition(),
		id: crypto.randomUUID(),
		manufacturer: archive.manufacturer,
		device_type: fixtureType(archive.model, allAttributes),
		name: archive.model,
		model: archive.model,
		model_asset: archive.modelAsset,
		color_calibration: calibration,
		mode: mode.getAttribute("Name") || `Mode ${modeIndex + 1}`,
		footprint: footprint.value,
		heads,
		physical: {
			pan_range_degrees: physicalRange(heads, "pan"),
			tilt_range_degrees: physicalRange(heads, "tilt"),
		},
	};
}

export async function importGdtfData(
	data: ArrayBuffer | Uint8Array,
	fileName: string,
): Promise<FixtureDefinition[]> {
	const archive = await loadGdtfArchive(data, fileName);
	const calibration = colorCalibration(archive.root);
	const modes = [...archive.root.querySelectorAll("DMXModes > DMXMode")];
	return (modes.length ? modes : [archive.root]).map((mode, modeIndex) =>
		modeDefinition(archive, mode, modeIndex, calibration),
	);
}

export async function importGdtf(file: File) {
	return importGdtfData(await file.arrayBuffer(), file.name);
}
