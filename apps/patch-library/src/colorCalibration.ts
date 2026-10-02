import type {
	ColorRecipeMeasurement,
	OpticalProvenance,
} from "./fixtureProfile";

/** Exact authored source; never rewritten when a fixture is replaced. */
export interface NativeColorIdentity {
	profile_id: string;
	profile_revision: number;
	profile_digest: string;
	mode_id: string;
	head_id: string;
	path_id: string;
	model_revision: number;
	native_layout_signature: string;
}
export interface InstalledEmitterCalibration {
	emitter_id: string;
	output_gain: number;
	provenance: OpticalProvenance;
}
export interface InstalledColorPathCalibration {
	source_identity: NativeColorIdentity;
	emitters: InstalledEmitterCalibration[];
	measurements: ColorRecipeMeasurement[];
}
export interface InstalledColorCalibration {
	version: number;
	revision: number;
	paths: InstalledColorPathCalibration[];
}

const object = (v: unknown): v is Record<string, unknown> =>
	v != null && typeof v === "object" && !Array.isArray(v);
const uuid = (v: unknown) =>
	typeof v === "string" &&
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v) &&
	v !== "00000000-0000-0000-0000-000000000000";
const hash = (v: unknown) => typeof v === "string" && /^[0-9a-f]{64}$/.test(v);
const u32 = (v: unknown) =>
	typeof v === "number" && Number.isInteger(v) && v >= 0 && v <= 0xffff_ffff;
const output = (v: unknown) =>
	typeof v === "number" && Number.isFinite(Math.fround(v)) && v >= 0;
const evidence = (v: unknown) =>
	object(v) &&
	u32(v.revision) &&
	["unknown", "estimated", "manufacturer", "measured"].includes(
		v.quality as string,
	) &&
	(v.source == null ||
		(typeof v.source === "string" &&
			new TextEncoder().encode(v.source).length <= 1024)) &&
	(!["manufacturer", "measured"].includes(v.quality as string) ||
		(typeof v.source === "string" && Boolean(v.source.trim())));

export function nativeColorIdentityError(value: unknown): string | null {
	if (
		!object(value) ||
		![value.profile_id, value.mode_id, value.head_id, value.path_id].every(
			uuid,
		) ||
		!u32(value.profile_revision) ||
		!u32(value.model_revision) ||
		!hash(value.profile_digest) ||
		!hash(value.native_layout_signature)
	)
		return "Invalid installed Color source identity.";
	return null;
}

/** Structural checks only: the backend compares the selected immutable profile at authoring. */
export function colorCalibrationError(value: unknown): string | null {
	if (value == null) return null;
	if (!object(value) || value.version !== 1 || !u32(value.revision))
		return "Unsupported installed Color calibration version or revision.";
	if (
		!Array.isArray(value.paths) ||
		!value.paths.length ||
		value.paths.length > 512
	)
		return "Installed Color calibration requires 1–512 paths.";
	const heads = new Set<string>(),
		paths = new Set<string>();
	let source: string | null = null,
		entries = 0;
	for (const path of value.paths) {
		if (!object(path) || !object(path.source_identity))
			return "Missing installed Color calibration source identity.";
		const id = path.source_identity;
		if (
			![id.profile_id, id.mode_id, id.head_id, id.path_id].every(uuid) ||
			!u32(id.profile_revision) ||
			!u32(id.model_revision) ||
			!hash(id.profile_digest) ||
			!hash(id.native_layout_signature) ||
			heads.has(id.head_id as string) ||
			paths.has(id.path_id as string)
		)
			return "Invalid or duplicate installed Color calibration identity.";
		heads.add(id.head_id as string);
		paths.add(id.path_id as string);
		const nextSource = JSON.stringify([
			id.profile_id,
			id.profile_revision,
			id.profile_digest,
			id.mode_id,
			id.model_revision,
		]);
		if (source !== null && source !== nextSource)
			return "Installed Color paths must refer to one profile, mode and optical model revision.";
		source = nextSource;
		if (
			!Array.isArray(path.emitters) ||
			!Array.isArray(path.measurements) ||
			(!path.emitters.length && !path.measurements.length)
		)
			return "Installed Color calibration path has no observations.";
		const emitters = new Set<string>(),
			recipes = new Set<string>();
		for (const e of path.emitters) {
			entries++;
			if (
				!object(e) ||
				!uuid(e.emitter_id) ||
				emitters.has(e.emitter_id as string) ||
				!output(e.output_gain) ||
				!evidence(e.provenance)
			)
				return "Invalid installed emitter gain or provenance.";
			emitters.add(e.emitter_id as string);
		}
		for (const m of path.measurements) {
			if (
				!object(m) ||
				!Array.isArray(m.recipe) ||
				!object(m.xyz) ||
				![m.xyz.x, m.xyz.y, m.xyz.z].every(output) ||
				!evidence(m.provenance)
			)
				return "Invalid installed Color measurement or provenance.";
			entries += 1 + m.recipe.length;
			const controls = new Set<string>();
			for (const v of m.recipe) {
				if (
					!object(v) ||
					!uuid(v.channel_id) ||
					!uuid(v.function_id) ||
					!u32(v.raw) ||
					controls.has(v.channel_id as string)
				)
					return "Invalid installed Color recipe.";
				controls.add(v.channel_id as string);
			}
			const recipe = JSON.stringify(
				m.recipe
					.map((v) => [v.channel_id, v.function_id, v.raw])
					.sort((a, b) => String(a[0]).localeCompare(String(b[0]))),
			);
			if (recipes.has(recipe))
				return "Installed Color calibration repeats a native recipe.";
			recipes.add(recipe);
		}
		if (entries > 65536)
			return "Installed Color calibration exceeds 65536 observation entries.";
	}
	return null;
}
