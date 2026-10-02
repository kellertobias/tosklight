import { Button, NumberField, SelectField, TextField } from "@tosklight/ui";
import type { ChannelFunction, PhysicalMappingCalibration } from "../fixtureProfile";
import {
	emptyPhysicalMapping,
	hasZoomDegreeMapping,
	nextPhysicalMappingSample,
	physicalMappingErrors,
	physicalMappingSamples,
	previewPhysicalValue,
} from "../sheet/fixtureProfileModel/physicalMapping";

function number(value: number): string {
	return Number.isFinite(value) ? Number(value.toPrecision(6)).toString() : "—";
}

function PhysicalMappingPreview({ fn }: { fn: ChannelFunction }) {
	if (fn.behavior.type !== "continuous") return null;
	const midpointRaw = Math.floor((fn.dmx_from + fn.dmx_to) / 2);
	const midpoint = previewPhysicalValue(fn, midpointRaw);
	if (midpoint === null)
		return <p className="field-hint">Correct the mapping before previewing it.</p>;
	const samples = physicalMappingSamples(fn);
	const low = Math.min(fn.behavior.physical_min, fn.behavior.physical_max);
	const span = Math.abs(fn.behavior.physical_max - fn.behavior.physical_min);
	const points = samples.map((sample) => ({
		x: 12 + ((sample.raw - fn.dmx_from) / (fn.dmx_to - fn.dmx_from)) * 376,
		y: 82 - ((sample.physical - low) / span) * 70,
	}));
	const unit = fn.behavior.unit?.trim() || "unspecified unit";
	return (
		<figure className="fixture-physical-mapping-preview">
			<svg viewBox="0 0 400 94" role="img" aria-label="Raw DMX to physical value curve">
				<path d="M12 10 V82 H388" fill="none" stroke="var(--muted)" />
				<polyline points={points.map((point) => `${point.x},${point.y}`).join(" ")}
					fill="none" stroke="var(--accent, #49c7bc)" strokeWidth="2" />
				{points.map((point, index) => <circle key={index} cx={point.x} cy={point.y} r="3"
					fill="var(--accent, #49c7bc)" />)}
			</svg>
			<figcaption>
				<span>Raw {fn.dmx_from} → {number(fn.behavior.physical_min)} {unit}</span>
				<output aria-label="Physical mapping midpoint">Raw {midpointRaw} → {number(midpoint)} {unit}</output>
				<span>Raw {fn.dmx_to} → {number(fn.behavior.physical_max)} {unit}</span>
			</figcaption>
		</figure>
	);
}

/** Portable fixture data authoring, within the existing function details. */
export function PhysicalMappingEditor({
	functionValue: fn,
	maximumRaw,
	onChange,
}: {
	functionValue: ChannelFunction;
	maximumRaw: number;
	onChange(fn: ChannelFunction): void;
}) {
	if (fn.behavior.type !== "continuous") return null;
	const mapping = fn.physical_mapping ?? emptyPhysicalMapping();
	const samples = mapping.samples ?? [];
	const setMapping = (patch: Partial<PhysicalMappingCalibration>) =>
		onChange({ ...fn, physical_mapping: { ...emptyPhysicalMapping(), ...mapping, ...patch } });
	const errors = physicalMappingErrors(fn, maximumRaw);
	const endpoints = [
		{ raw: fn.dmx_from, physical: fn.behavior.physical_min },
		{ raw: fn.dmx_to, physical: fn.behavior.physical_max },
	];
	const nextSample = errors.length ? null : nextPhysicalMappingSample(fn);
	const addSample = () => {
		if (!nextSample) return;
		setMapping({
			samples: [
				...samples.slice(0, nextSample.index),
				nextSample.sample,
				...samples.slice(nextSample.index),
			],
		});
	};
	return (
		<section className="fixture-physical-mapping" aria-label="Physical mapping calibration">
			<h4>Physical mapping</h4>
			<p className="field-hint">The function endpoints define its scale. Add samples for a measured or documented curve; an empty sample list uses a straight line. Missing data remains Unknown.</p>
			<p className="field-hint">Calibration samples are saved and previewed here. They do not yet change live output.</p>
			<div className="fixture-physical-mapping-fields">
				<SelectField label="Mapping quality" ariaLabel="Mapping quality" value={mapping.quality ?? "unknown"}
					options={[
						{ value: "unknown", label: "Unknown" },
						{ value: "estimated", label: "Estimated" },
						{ value: "manufacturer", label: "Manufacturer" },
						{ value: "measured", label: "Measured" },
					]}
					onChange={(quality) => setMapping({ quality: quality as PhysicalMappingCalibration["quality"] })} />
				<TextField label="Mapping source" value={mapping.source ?? ""}
					placeholder="Manual, measurement or instrument"
					onChange={(event) => setMapping({ source: event.target.value || null })} />
				<NumberField label="Mapping revision" min={0} max={0xffff_ffff} value={mapping.revision ?? 0}
					onChange={(event) => setMapping({ revision: Number(event.target.value) })} />
				{(hasZoomDegreeMapping(fn) || mapping.opening_convention != null) && (
					<SelectField label="Zoom opening convention" ariaLabel="Zoom opening convention" value={mapping.opening_convention ?? ""}
						options={[
							{ value: "", label: "Unknown" },
							{ value: "beam", label: "Beam angle" },
							{ value: "field", label: "Field angle" },
						]}
						onChange={(opening) => setMapping({ opening_convention: opening ? opening as "beam" | "field" : null })} />
				)}
			</div>
			{samples.length > 0 && (
				<div className="fixture-physical-mapping-samples">
					{samples.map((sample, index) => (
						<div key={index} className="fixture-physical-mapping-sample">
							<NumberField label={`Sample ${index + 1} raw`} min={fn.dmx_from} max={fn.dmx_to} value={sample.raw}
								onChange={(event) => setMapping({ samples: samples.map((item, itemIndex) => itemIndex === index ? { ...item, raw: Number(event.target.value) } : item) })} />
							<NumberField label={`Sample ${index + 1} physical`} allowDecimal value={sample.physical}
								onChange={(event) => setMapping({ samples: samples.map((item, itemIndex) => itemIndex === index ? { ...item, physical: Number(event.target.value) } : item) })} />
							<Button disabled={index === 0 || index === samples.length - 1} aria-label={`Remove sample ${index + 1}`}
								onClick={() => setMapping({ samples: samples.filter((_, itemIndex) => itemIndex !== index) })}>Remove</Button>
						</div>
					))}
				</div>
			)}
			<div className="fixture-physical-mapping-actions">
				{samples.length ? <>
					<Button disabled={!nextSample} onClick={addSample}>Add intermediate sample</Button>
					<Button onClick={() => setMapping({ samples: samples.length > 2 ? [endpoints[0], ...samples.slice(1, -1), endpoints[1]] : endpoints })}>Use function endpoints</Button>
					<Button onClick={() => setMapping({ samples: [] })}>Use linear mapping</Button>
				</> : <Button onClick={() => setMapping({ samples: endpoints })}>Use sampled mapping</Button>}
				{fn.physical_mapping && <Button onClick={() => onChange({ ...fn, physical_mapping: null })}>Clear mapping calibration</Button>}
			</div>
			{samples.length > 0 && !nextSample && errors.length === 0 && (
				<p className="field-hint">No additional sample fits: the raw interval or stored physical precision is exhausted.</p>
			)}
			{errors.length > 0 ? <ul role="alert" className="fixture-physical-mapping-errors">{errors.map((error) => <li key={error}>{error}</li>)}</ul> : <PhysicalMappingPreview fn={fn} />}
		</section>
	);
}
