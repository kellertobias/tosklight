/** Portable per-physical-instance data; current live output does not apply these offsets. */
export interface InstalledPositionCalibration {
	revision: number;
	quality: "unknown" | "estimated" | "manufacturer" | "measured";
	source?: string | null;
	pan_zero_degrees: number;
	tilt_zero_degrees: number;
	axis_overrides?: InstalledAxisOverrides | null;
}

export function defaultPositionCalibration(): InstalledPositionCalibration {
	return {
		revision: 0,
		quality: "unknown",
		source: null,
		pan_zero_degrees: 0,
		tilt_zero_degrees: 0,
	};
}

export function positionCalibrationError(
	value: InstalledPositionCalibration | null | undefined,
): string | null {
	if (value == null) return null;
	const overridesError = axisOverridesError(value.axis_overrides);
	if (overridesError) return overridesError;
	if (
		!Number.isFinite(Math.fround(value.pan_zero_degrees)) ||
		!Number.isFinite(Math.fround(value.tilt_zero_degrees))
	)
		return "Position calibration zero offsets must be finite degrees.";
	if (
		!Number.isInteger(value.revision) ||
		value.revision < 0 ||
		value.revision > 0xffff_ffff
	)
		return "Calibration revision must be a whole number from 0 to 4294967295.";
	if (
		!["unknown", "estimated", "manufacturer", "measured"].includes(
			value.quality,
		)
	)
		return "Choose a valid calibration quality.";
	if (
		value.source != null &&
		new TextEncoder().encode(value.source).length > 1024
	)
		return "Calibration source must be at most 1024 bytes.";
	if (
		(value.quality === "manufacturer" || value.quality === "measured") &&
		!value.source?.trim()
	)
		return "Manufacturer and measured calibration need a source.";
	return null;
}

export interface PositionCalibrationIdentity {
	profile_id: string;
	mode_id: string;
	geometry_digest: string;
}
export interface InstalledAxisCalibration {
	node_id: string;
	zero_degrees: number;
	invert: boolean;
}
export interface InstalledAxisOverrides {
	version: number;
	source_identity: PositionCalibrationIdentity;
	axes: InstalledAxisCalibration[];
}
export interface PositionCalibrationContext {
	identity: PositionCalibrationIdentity;
	axes: readonly { node_id: string; name: string; role: "pan" | "tilt" }[];
}
const object = (v: unknown): v is Record<string, unknown> =>
	v != null && typeof v === "object" && !Array.isArray(v);
const validUuid = (v: unknown) =>
	typeof v === "string" &&
	/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v) &&
	v !== "00000000-0000-0000-0000-000000000000";
export function positionCalibrationIdentityError(v: unknown): string | null {
	return object(v) &&
		validUuid(v.profile_id) &&
		validUuid(v.mode_id) &&
		typeof v.geometry_digest === "string" &&
		/^[0-9a-f]{64}$/.test(v.geometry_digest)
		? null
		: "Invalid Position calibration source identity.";
}
export function axisOverridesError(v: unknown): string | null {
	if (v == null) return null;
	if (
		!object(v) ||
		v.version !== 1 ||
		positionCalibrationIdentityError(v.source_identity) ||
		!Array.isArray(v.axes) ||
		v.axes.length < 1 ||
		v.axes.length > 4096
	)
		return "Invalid Position axis override contract.";
	const seen = new Set<string>();
	for (const a of v.axes) {
		if (
			!object(a) ||
			!validUuid(a.node_id) ||
			seen.has(a.node_id as string) ||
			typeof a.zero_degrees !== "number" ||
			!Number.isFinite(Math.fround(a.zero_degrees)) ||
			typeof a.invert !== "boolean"
		)
			return "Position overrides need unique axes, finite zero offsets and explicit inversion.";
		seen.add(a.node_id as string);
	}
	return null;
}
export function positionCalibrationIsStale(
	v: InstalledAxisOverrides | null | undefined,
	context: PositionCalibrationContext | null | undefined,
): boolean {
	return (
		!!v &&
		(!context ||
			v.source_identity.profile_id !== context.identity.profile_id ||
			v.source_identity.mode_id !== context.identity.mode_id ||
			v.source_identity.geometry_digest !== context.identity.geometry_digest ||
			v.axes.some((a) => !context.axes.some((n) => n.node_id === a.node_id)))
	);
}
