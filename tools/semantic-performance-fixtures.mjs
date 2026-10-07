// Real fixture-package capability discovery and a deterministic synthetic rig for the semantic
// performance workloads (TL-564). Packages are read-only inputs from assets/fixture-library.
import fs from "node:fs";
import path from "node:path";
import { inflateRawSync } from "node:zlib";
import {
	createSeededRandom,
	fileSha256,
	semanticContractRepositoryRoot,
	uuidV5,
	workloadNamespace,
} from "./semantic-performance-contract.mjs";

export const DEFAULT_FIXTURE_LIBRARY = "assets/fixture-library";
export const POINT_FIXTURE_TYPE = "position_point";

// ---------------------------------------------------------------------------------------------
// Package reading

/** Read one stored or deflated entry from a zip archive without external dependencies. */
export function readZipEntry(file, entryName) {
	const archive = fs.readFileSync(file);
	let end = archive.length - 22;
	while (end >= 0 && archive.readUInt32LE(end) !== 0x06054b50) end -= 1;
	if (end < 0) throw new Error(`${file} is not a zip archive`);
	const entries = archive.readUInt16LE(end + 10);
	let offset = archive.readUInt32LE(end + 16);
	for (let index = 0; index < entries; index += 1) {
		if (archive.readUInt32LE(offset) !== 0x02014b50)
			throw new Error(`${file} has a corrupt central directory`);
		const method = archive.readUInt16LE(offset + 10);
		const compressedSize = archive.readUInt32LE(offset + 20);
		const nameLength = archive.readUInt16LE(offset + 28);
		const extraLength = archive.readUInt16LE(offset + 30);
		const commentLength = archive.readUInt16LE(offset + 32);
		const localOffset = archive.readUInt32LE(offset + 42);
		const name = archive.toString("utf8", offset + 46, offset + 46 + nameLength);
		if (name === entryName) {
			const localName = archive.readUInt16LE(localOffset + 26);
			const localExtra = archive.readUInt16LE(localOffset + 28);
			const start = localOffset + 30 + localName + localExtra;
			const data = archive.subarray(start, start + compressedSize);
			if (method === 0) return data;
			if (method === 8) return inflateRawSync(data);
			throw new Error(`${file}:${entryName} uses unsupported zip method ${method}`);
		}
		offset += 46 + nameLength + extraLength + commentLength;
	}
	throw new Error(`${file} has no ${entryName}`);
}

/** Load every `.toskfixture` package; `packages` optionally restricts the base names. */
export function loadFixtureLibrary({
	directory = DEFAULT_FIXTURE_LIBRARY,
	packages,
	root = semanticContractRepositoryRoot,
} = {}) {
	const absolute = path.resolve(root, directory);
	const names = (packages ?? fs
		.readdirSync(absolute)
		.filter((name) => name.endsWith(".toskfixture"))
		.map((name) => name.replace(/\.toskfixture$/u, "")))
		.slice()
		.sort();
	return names.map((name) => {
		const file = path.join(absolute, `${name}.toskfixture`);
		const document = JSON.parse(readZipEntry(file, "fixture.json").toString("utf8"));
		return {
			packageName: name,
			file: path.relative(root, file),
			sha256: fileSha256(file),
			profile: document.profile,
		};
	});
}

// ---------------------------------------------------------------------------------------------
// Capability classification

function channelAttribute(channel) {
	return channel.fixture_attribute ?? channel.attribute ?? "";
}

function zoomPhysics(channels) {
	for (const channel of channels) {
		for (const fn of channel.functions ?? []) {
			if (fn.attribute !== "zoom") continue;
			const unit = String(fn.behavior?.unit ?? "").trim().toLowerCase();
			if (["deg", "degree", "degrees", "°"].includes(unit))
				return {
					physical: "degrees",
					quality: fn.physical_mapping?.quality ?? "unknown",
					convention: fn.physical_mapping?.opening_convention ?? null,
				};
		}
	}
	return { physical: "unmapped", quality: "unknown", convention: null };
}

/** Per-head semantic capabilities of one profile mode, in profile head order. */
export function classifyModeHeads(profile, modeId) {
	const mode = profile.modes?.find((candidate) => candidate.id === modeId);
	if (!mode) throw new Error(`${profile.name} has no mode ${modeId}`);
	return mode.heads.map((head, headIndex) => {
		const channels = mode.channels.filter((channel) => channel.head_id === head.id);
		const attributes = new Set(channels.map(channelAttribute));
		const has = (...names) => names.every((name) => attributes.has(`color.${name}`));
		const cmy = has("cyan", "magenta", "yellow");
		const rgb = has("red", "green", "blue");
		const wheel = [...attributes].some((attribute) => /^color\.wheel\.\d+$/u.test(attribute));
		const colorClass = cmy
			? "cmy"
			: rgb && has("white")
				? "rgbw"
				: rgb
					? "rgb"
					: wheel
						? "wheel"
						: null;
		const zoom = attributes.has("zoom") ? zoomPhysics(channels) : null;
		return {
			headId: head.id,
			headIndex,
			headName: head.name,
			masterShared: Boolean(head.master_shared),
			angle: attributes.has("pan") && attributes.has("tilt"),
			colorClass,
			colorWheel: wheel,
			uv: attributes.has("color.uv"),
			focus: attributes.has("focus"),
			zoom,
			point: [...attributes].some((attribute) => attribute.startsWith("point.position.")),
		};
	});
}

export function isPointProfile(profile) {
	return profile?.fixture_type === POINT_FIXTURE_TYPE;
}

// ---------------------------------------------------------------------------------------------
// Deterministic synthetic rig (offline input only; a live run reads the real patch back)

/** Default mixed rig. Every entry names a real shipped package and exact mode. */
export const DEFAULT_SEMANTIC_RIG = Object.freeze([
	{ key: "dls", packageName: "robe--robin-dls-profile", mode: "Mode 1", count: 24 },
	{ key: "cmy", packageName: "claypaky--stage-zoom-1200", mode: "16-Channel", count: 12 },
	{ key: "wheel", packageName: "cameo--auro-spot-z300", mode: "17-Channel", count: 12 },
	{ key: "ledwash", packageName: "robe--robin-600x-ledwash", mode: "Mode 1", count: 12 },
	{ key: "rgb", packageName: "generic--rgb-led", mode: "RGBD 8-bit dimmer last", count: 16 },
	{ key: "rgbw", packageName: "generic--rgbw-led", mode: "RGBWD 8-bit dimmer last", count: 16 },
	{ key: "rgbwauv", packageName: "generic--rgbwauv-led", mode: "RGBWAUD 8-bit dimmer last", count: 8 },
	{ key: "rootpar", packageName: "cameo--root-par-6", mode: "D7CH — Delay Off, virtual dimmer", count: 8 },
	{ key: "sunstrip", packageName: "showtec--sunstrip-led-rgb-42206", mode: "30 Channel", count: 4 },
].map(Object.freeze));
export const DEFAULT_POINT_PACKAGE = Object.freeze({
	packageName: "tosklight--3d-point",
	mode: "Position 16 bit",
});

/**
 * Default moving-mount plan: which movers are slaved to moving mount Points. Aim Points are
 * chosen later by the workload builder from Points that are not mounts.
 */
export const DEFAULT_MOUNT_PLAN = Object.freeze({
	sharedGroups: 2,
	sharedGroupSize: 6,
	separateMounts: 6,
	aimPoints: 1 + 2 + 6,
});

/**
 * Build a PatchSnapshot-shaped structure from real packages with deterministic identities.
 * It carries the same fields the workload builder reads from a live `/api/v2/patch` snapshot.
 */
export function createSyntheticSemanticPatch({
	seed,
	library,
	rig = DEFAULT_SEMANTIC_RIG,
	points = DEFAULT_POINT_PACKAGE,
	mounts = DEFAULT_MOUNT_PLAN,
}) {
	const namespace = workloadNamespace(seed);
	const byPackage = new Map(library.map((entry) => [entry.packageName, entry]));
	const profiles = new Map();
	const fixtures = [];
	const shortfalls = [];
	let fixtureNumber = 1;
	const resolveMode = (entry) => {
		const pkg = byPackage.get(entry.packageName);
		const mode = pkg?.profile.modes.find((candidate) => candidate.name === entry.mode);
		if (!pkg || !mode) {
			shortfalls.push({
				kind: "rig-entry",
				key: entry.key ?? entry.packageName,
				reason: pkg ? `mode ${entry.mode} is not in the package` : "package is not in the library",
			});
			return null;
		}
		profiles.set(`${pkg.profile.id}:${pkg.profile.revision}`, pkg);
		return { pkg, mode };
	};
	const addFixture = (entry, resolved, index, positionMaster = null) => {
		const { pkg, mode } = resolved;
		const fixtureId = uuidV5(namespace, `fixture:${entry.key ?? entry.packageName}:${index}`);
		const logicalHeads = mode.heads
			.map((head, headIndex) => ({ head, headIndex }))
			.filter(({ head }) => !head.master_shared)
			.map(({ head, headIndex }) => ({
				profile_head_id: head.id,
				head_index: headIndex,
				fixture_id: uuidV5(namespace, `head:${fixtureId}:${headIndex}`),
			}));
		const fixture = {
			fixture_id: fixtureId,
			fixture_number: fixtureNumber,
			name: `${pkg.profile.short_name || pkg.profile.name} ${index + 1}`,
			profile_id: pkg.profile.id,
			profile_revision: pkg.profile.revision,
			mode_id: mode.id,
			logical_heads: logicalHeads,
			position_master: positionMaster,
		};
		fixtureNumber += 1;
		fixtures.push(fixture);
		return fixture;
	};

	for (const entry of rig) {
		const resolved = resolveMode(entry);
		if (!resolved) continue;
		for (let index = 0; index < entry.count; index += 1) addFixture(entry, resolved, index);
	}

	const pointMode = resolveMode({ key: "point", ...points });
	const random = createSeededRandom(seed, "mount-plan");
	const movers = random.shuffle(
		fixtures.filter((fixture) => {
			const pkg = profiles.get(`${fixture.profile_id}:${fixture.profile_revision}`);
			return classifyModeHeads(pkg.profile, fixture.mode_id).some((head) => head.angle);
		}),
	);
	const mountPoints = [];
	if (pointMode) {
		let pointIndex = 0;
		const addPoint = (role) => {
			const point = addFixture({ key: `point-${role}` }, pointMode, pointIndex);
			pointIndex += 1;
			return point;
		};
		let cursor = 0;
		const takeMovers = (count, label) => {
			const taken = movers.slice(cursor, cursor + count);
			cursor += taken.length;
			if (taken.length < count)
				shortfalls.push({ kind: "mount-plan", key: label, requested: count, realized: taken.length });
			return taken;
		};
		for (let group = 0; group < mounts.sharedGroups; group += 1) {
			const mount = addPoint("mount");
			mountPoints.push(mount.fixture_id);
			for (const fixture of takeMovers(mounts.sharedGroupSize, `shared-group-${group}`))
				fixture.position_master = mount.fixture_id;
		}
		for (const fixture of takeMovers(mounts.separateMounts, "separate")) {
			const mount = addPoint("mount");
			mountPoints.push(mount.fixture_id);
			fixture.position_master = mount.fixture_id;
		}
		for (let aim = 0; aim < mounts.aimPoints; aim += 1) addPoint("aim");
	}

	return {
		identity_source: "synthetic-deterministic",
		seed: String(seed),
		fixtures,
		profile_revisions: [...profiles.values()]
			.sort((left, right) => left.profile.id.localeCompare(right.profile.id))
			.map((pkg) => ({
				profile_id: pkg.profile.id,
				profile_revision: pkg.profile.revision,
				content_digest: pkg.sha256,
				manufacturer: pkg.profile.manufacturer,
				name: pkg.profile.name,
				fixture_type: pkg.profile.fixture_type,
				profile_snapshot: pkg.profile,
			})),
		rig: {
			entries: rig.map((entry) => ({ ...entry })),
			points: { ...points },
			mounts: { ...mounts },
			packages: [...profiles.values()]
				.map((pkg) => ({
					package: pkg.packageName,
					file: pkg.file,
					sha256: pkg.sha256,
					profile_id: pkg.profile.id,
					profile_revision: pkg.profile.revision,
				}))
				.sort((left, right) => left.package.localeCompare(right.package)),
			shortfalls,
		},
	};
}
