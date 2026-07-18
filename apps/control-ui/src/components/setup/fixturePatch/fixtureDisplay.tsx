import type { FixtureDefinition } from "../../../api/types";
import { isDmxPatchable } from "../patchUtils";

export function formatRotation(
	rotation: { x: number; y: number; z: number } | undefined,
) {
	return (["x", "y", "z"] as const)
		.map((axis) => `${Number((rotation?.[axis] ?? 0).toFixed(3))}°`)
		.join(" / ");
}

export function FixtureTypeIcon({ type }: { type: string }) {
	const kind = fixtureTypeKind(type);
	return (
		<span
			className="fixture-type-icon"
			title={type || "other"}
			role="img"
			aria-label={`Type: ${type || "other"}`}
		>
			<svg viewBox="0 0 24 24" aria-hidden="true">
				<FixtureTypeGlyph kind={kind} />
			</svg>
		</span>
	);
}

function FixtureTypeGlyph({
	kind,
}: {
	kind: ReturnType<typeof fixtureTypeKind>;
}) {
	if (kind === "atmosphere")
		return (
			<path d="M3 8c3-3 5 3 8 0s5 3 8 0M3 13c3-3 5 3 8 0s5 3 8 0M5 18c2-2 4 2 6 0s4 2 6 0" />
		);
	if (kind === "moving")
		return (
			<>
				<path d="M8 4h8l2 5-3 5H9L6 9zM12 14v4M7 20h10" />
				<path d="M16 6l5-2M17 9h5" />
			</>
		);
	if (kind === "wash")
		return (
			<>
				<path d="M8 4h8l2 5-3 5H9L6 9zM12 14v5M7 21h10" />
				<path d="M5 3 2 6M19 3l3 3" />
			</>
		);
	if (kind === "profile")
		return <path d="M4 7h7l3 4-3 4H4zM14 9l7-3v10l-7-3" />;
	if (kind === "strobe") return <path d="m13 2-8 12h6l-1 8 9-13h-6z" />;
	if (kind === "media")
		return (
			<>
				<rect x="3" y="4" width="18" height="14" rx="2" />
				<path d="m10 8 5 3-5 3zM8 21h8" />
			</>
		);
	if (kind === "pixels")
		return (
			<>
				<rect x="3" y="3" width="7" height="7" />
				<rect x="14" y="3" width="7" height="7" />
				<rect x="3" y="14" width="7" height="7" />
				<rect x="14" y="14" width="7" height="7" />
			</>
		);
	if (kind === "dimmer")
		return (
			<>
				<circle cx="12" cy="12" r="8" />
				<path d="M12 4a8 8 0 0 0 0 16zM12 1v2M12 21v2M1 12h2M21 12h2" />
			</>
		);
	return (
		<>
			<path d="M12 3 21 12 12 21 3 12z" />
			<circle cx="12" cy="12" r="2" />
		</>
	);
}

function fixtureTypeKind(type: string) {
	const value = type.toLowerCase();
	if (/fog|haze|fan/.test(value)) return "atmosphere";
	if (/media|video/.test(value)) return "media";
	if (/pixel|strip|matrix/.test(value)) return "pixels";
	if (/strobe/.test(value)) return "strobe";
	if (/moving|mover|beam/.test(value)) return "moving";
	if (/wash/.test(value)) return "wash";
	if (/profile|spot/.test(value)) return "profile";
	if (/dimmer|relay/.test(value)) return "dimmer";
	return "other";
}

export function MultiPatchBranch({ last }: { last: boolean }) {
	return (
		<span className="multipatch-branch" aria-hidden="true">
			<svg viewBox="0 0 28 42" aria-hidden="true">
				<path d={last ? "M7 0v20q0 6 6 6h12" : "M7 0v42M7 20q0 6 6 6h12"} />
			</svg>
		</span>
	);
}

export function FixtureDetails({
	definition,
}: {
	definition: FixtureDefinition;
}) {
	return (
		<div className="fixture-details">
			<strong>
				{isDmxPatchable(definition)
					? `${definition.footprint} DMX channels`
					: "Visual only · no DMX patch"}
			</strong>
			<span>{definition.device_type}</span>
			<span>
				{definition.heads.length} head{definition.heads.length === 1 ? "" : "s"}
			</span>
			<span>Revision {definition.revision}</span>
			{definition.physical.width_millimetres && (
				<span>
					{definition.physical.width_millimetres} ×{" "}
					{definition.physical.height_millimetres ?? "?"} ×{" "}
					{definition.physical.depth_millimetres ?? "?"} mm
				</span>
			)}
		</div>
	);
}
