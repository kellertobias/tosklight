import { Button, FormField, Input } from "@tosklight/ui/controls";
import { type EncoderSectionItem, type EncoderSectionSurface } from "@tosklight/ui/encoders";
import type { ReactNode } from "react";

export type MockupSurface = EncoderSectionSurface;

export function MockupSection({ title, description, children, className = "" }: {
	title: string; description?: string; children: ReactNode; className?: string;
}) {
	return <section className={`fam-panel ${className}`} aria-label={title}>
		<header className="fam-panel-heading"><h2>{title}</h2>{description && <p>{description}</p>}</header>
		{children}
	</section>;
}

export function MockupChoices<T extends string>({ label, value, choices, onChange }: {
	label: string; value: T; choices: readonly { value: T; label: string }[]; onChange: (value: T) => void;
}) {
	return <div className="fam-choices" role="group" aria-label={label}>
		{choices.map((choice) => <Button key={choice.value} size="compact" active={choice.value === value}
			aria-pressed={choice.value === value} onClick={() => onChange(choice.value)}>{choice.label}</Button>)}
	</div>;
}

export function mockupEncoder(id: string, label: string, value: number, slot: number, unit = "%", minimum = 0, maximum = 100, step = 1): EncoderSectionItem {
	return { id, slot, value, minimum, maximum, inputScale: 1, slowStep: step, fastStep: step * 10,
		accentColor: "#1b6978", target: { id, label, display: `${value.toFixed(unit === "Duv" ? 4 : unit === "m" ? 2 : unit === "Hz" ? 1 : 0)}${unit === "K" || unit === "Hz" || unit === "m" || unit === "Duv" ? " " : ""}${unit}` },
		mode: "Coarse · Fine" };
}

export function MockupSlider({ label, value, onChange, minimum = 0, maximum = 100, step = 1, unit = "%" }: {
	label: string; value: number; onChange: (value: number) => void; minimum?: number; maximum?: number; step?: number; unit?: string;
}) {
	const id = `fam-slider-${label.toLowerCase().replace(/[^a-z0-9]+/g, "-")}`;
	return <FormField label={label} htmlFor={id}><div className="fam-slider-row">
		<Input id={id} type="range" min={minimum} max={maximum} step={step} value={value} onChange={(event) => onChange(Number(event.target.value))} />
		<output htmlFor={id}>{value.toFixed(step < 1 ? 2 : 0)}{unit}</output>
	</div></FormField>;
}
