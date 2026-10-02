import { Button, FormLayout, NumberField, SelectField } from "@tosklight/ui/controls";
import { useState } from "react";
import { MockupChoices, MockupSection, MockupSlider } from "./MockupControls";

const defaults = { panMin: -270, panMax: 270, tiltMin: -135, tiltMax: 135, zoomMin: 8, zoomMax: 48, panOffset: 0, tiltOffset: 0 };
const fields = [
	["panMin", "Pan minimum", "°"], ["panMax", "Pan maximum", "°"],
	["tiltMin", "Tilt minimum", "°"], ["tiltMax", "Tilt maximum", "°"],
	["zoomMin", "Zoom minimum", "°"], ["zoomMax", "Zoom maximum", "°"],
] as const;

export function FixtureConfigurationMockup() {
	const [tab, setTab] = useState<"profile" | "installation">("profile");
	const [template, setTemplate] = useState("rgbw");
	const [values, setValues] = useState(defaults);
	const [invertPan, setInvertPan] = useState(false);
	const [invertTilt, setInvertTilt] = useState(false);
	const [requested, setRequested] = useState(60);
	const [message, setMessage] = useState("Fixture profile settings");
	const [response, setResponse] = useState("linear");
	const [emitterLevel, setEmitterLevel] = useState(100);
	const [selectedEmitter, setSelectedEmitter] = useState("Red");
	const emitters = template === "rgbal" ? ["Red", "Green", "Blue", "Amber", "Lime"] : template === "rgbcct" ? ["Red", "Green", "Blue", "Cold white", "Warm white"] : template === "cmy" ? ["Open white", "Cyan filter", "Magenta filter", "Yellow filter"] : ["Red", "Green", "Blue", "White"];
	const corrected = Math.max(values.panMin, Math.min(values.panMax, (invertPan ? -requested : requested) + values.panOffset));
	const native = Math.round((corrected - values.panMin) / Math.max(0.01, values.panMax - values.panMin) * 65535);
	const invalid = values.panMin >= values.panMax || values.tiltMin >= values.tiltMax || values.zoomMin >= values.zoomMax;
	return <div>
		<div className="fam-context-row"><MockupChoices label="Configuration layer" value={tab} choices={[{ value: "profile", label: "Fixture profile" }, { value: "installation", label: "Installed fixture" }]} onChange={setTab} /><span className="fam-chip">Example mover · 16-bit mode</span></div>
		<div className="fam-two-columns">
			<MockupSection title={tab === "profile" ? "Physical capabilities" : "Mounting calibration"} description={tab === "profile" ? "Describe what this lamp can physically produce." : "Correct this installation without changing any preset."}>
				{tab === "profile" ? <>
					<FormLayout columns={3}>{fields.map(([key, label, unit]) => <NumberField key={key} label={`${label} (${unit})`} value={values[key]} onValueChange={(value) => { if (Number.isFinite(Number(value))) setValues((current) => ({ ...current, [key]: Number(value) })); }} />)}</FormLayout>
					<p className={`fam-status ${invalid ? "is-approximate" : "is-matched"}`}>{invalid ? "Minimum must be below maximum." : "Manufacturer supplied · example ranges"}</p>
					<div className="fam-section-divider" />
					<SelectField label="Color system template" value={template} options={[{ value: "rgbw", label: "RGBW · simple additive" }, { value: "rgbcct", label: "RGB + cold / warm white" }, { value: "rgbal", label: "RGB + amber / lime" }, { value: "cmy", label: "CMY + color wheel" }]} onChange={(value) => { setTemplate(value); setSelectedEmitter(value === "cmy" ? "Open white" : "Red"); }} />
					<div className="fam-emitter-list" aria-label="Available emitters">{emitters.map((name) => <Button key={name} active={selectedEmitter === name} onClick={() => setSelectedEmitter(name)}>{name}</Button>)}</div>
					<FormLayout columns={2}><SelectField label="Response curve" value={response} options={[{ value: "linear", label: "Linear · estimated" }, { value: "measured", label: "Measured curve · example" }]} onChange={setResponse} /><MockupSlider label={`${selectedEmitter} maximum output`} value={emitterLevel} onChange={setEmitterLevel} /></FormLayout>
					<p className="fam-muted">Reference emitter coordinates are supplied by the template. Actual spectra and output measurements can replace these estimates.</p>
				</> : <>
					<div className="fam-inline"><Button aria-pressed={invertPan} active={invertPan} onClick={() => setInvertPan(!invertPan)}>Invert Pan</Button><Button aria-pressed={invertTilt} active={invertTilt} onClick={() => setInvertTilt(!invertTilt)}>Invert Tilt</Button></div>
					<FormLayout columns={2}><NumberField label="Pan zero offset (°)" value={values.panOffset} onValueChange={(value) => { if (Number.isFinite(Number(value))) setValues((current) => ({ ...current, panOffset: Number(value) })); }} /><NumberField label="Tilt zero offset (°)" value={values.tiltOffset} onValueChange={(value) => { if (Number.isFinite(Number(value))) setValues((current) => ({ ...current, tiltOffset: Number(value) })); }} /></FormLayout>
					<p className="fam-muted">Lamp 12 · mounted on Moving truss. Profile measurements stay separate from installation offsets.</p>
					<MockupSlider label="Requested pan" value={requested} onChange={setRequested} minimum={-270} maximum={270} unit="°" />
					<div className="fam-readouts"><div><small>Requested</small><strong>{requested}°</strong></div><div><small>Calibrated output</small><strong>{corrected}°</strong></div></div>
				</>}
				<Button variant="primary" disabled={invalid} onClick={() => setMessage("Configuration saved.")}>Save configuration</Button>
				<p role="status" className="fam-muted">{message}</p>
			</MockupSection>
			<div className="fam-sidebar">
				<MockupSection title="Mapping preview" description="Requested degrees → calibrated movement → DMX."><div className="fam-readouts"><div><small>Pan physical</small><strong>{corrected}°</strong></div><div><small>16-bit DMX</small><strong>{native >> 8} / {native & 255}</strong></div></div><p className="fam-muted">Pan / Pan fine · illustrative linear mapping</p></MockupSection>
				<MockupSection title="GDTF data quality"><ul className="fam-quality-list"><li><span className="fam-dot is-matched" />Movement ranges <b>Supplied</b></li><li><span className="fam-dot is-matched" />Emitter xyY <b>Supplied</b></li><li><span className="fam-dot is-approximate" />Spectral measurements <b>Missing</b></li><li><span className="fam-dot is-approximate" />Response curves <b>Estimated</b></li></ul><Button onClick={() => setMessage("Movement and emitter colors supplied; spectra missing.")}>Preview GDTF import</Button><p className="fam-muted">Complete missing data with manufacturer documentation or measurements.</p></MockupSection>
			</div>
		</div>
	</div>;
}
