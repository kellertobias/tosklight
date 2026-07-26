import type { CSSProperties, ReactNode } from "react";
import { HardwareEncoderDisplayView } from "./HardwareEncoderDisplay";
import { TouchEncoder } from "./TouchEncoder";

export type EncoderSectionSurface = "touch" | "hardware";

export interface EncoderSectionTarget {
	label: string;
	display: string;
	role?: string;
}

export interface EncoderSectionItem {
	id: string;
	slot: number;
	target?: EncoderSectionTarget;
	secondary?: EncoderSectionTarget;
	/** Normalized encoder value. Hardware absolute entry is converted to display points. */
	value: number;
	mode?: string;
	accentColor?: string;
	disabled?: boolean;
	indexed?: boolean;
	canRelease?: boolean;
}

export interface EncoderSectionModel {
	id: string;
	label: ReactNode;
	description?: ReactNode;
	encoders: readonly EncoderSectionItem[];
}

export interface EncoderSectionCallbacks {
	onRelativeChange?(
		id: string,
		delta: number,
		undoGroup?: string | null,
	): void;
	onAbsoluteChange?(id: string, value: number): void;
	onRangeChange?(id: string, points: number[]): void;
	onRelease?(id: string): void;
}

export interface EncoderSectionProps {
	model: EncoderSectionModel;
	surface: EncoderSectionSurface;
	callbacks?: EncoderSectionCallbacks;
	className?: string;
}

function normalized(value: number) {
	return Math.max(0, Math.min(1, value));
}

export function EncoderSection({
	model,
	surface,
	callbacks = {},
	className = "",
}: EncoderSectionProps) {
	return (
		<section
			className={`encoder-section ${surface}-encoder-section ${className}`.trim()}
			aria-labelledby={`encoder-section-${model.id}`}
			data-encoder-family={model.id}
			data-encoder-surface={surface}
			style={{
				minWidth: 0,
				minHeight: 0,
				display: "grid",
				gridTemplateRows: "auto minmax(0, 1fr)",
			}}
		>
			<header className="encoder-section-header">
				<strong id={`encoder-section-${model.id}`}>{model.label}</strong>
				{model.description != null && <small>{model.description}</small>}
			</header>
			<div
				className="encoder-section-items"
				style={
					{
						minWidth: 0,
						minHeight: 0,
						display: "grid",
						gridTemplateColumns: `repeat(${Math.max(model.encoders.length, 1)}, minmax(0, 1fr))`,
						gap: 5,
					} as CSSProperties
				}
			>
				{model.encoders.map((encoder) =>
					surface === "touch" ? (
						<TouchEncoder
							key={encoder.id}
							label={`Enc ${encoder.slot} · ${encoder.target?.label ?? "Unassigned"}`}
							display={encoder.target?.display ?? "—"}
							value={normalized(encoder.value)}
							disabled={encoder.disabled || !encoder.target}
							accentColor={encoder.accentColor}
							mode={encoder.mode}
							indexed={encoder.indexed}
							canRelease={encoder.canRelease}
							onStep={(delta, undoGroup) =>
								callbacks.onRelativeChange?.(encoder.id, delta, undoGroup)
							}
							onSet={(value) =>
								callbacks.onAbsoluteChange?.(encoder.id, value)
							}
							onSetRange={
								callbacks.onRangeChange
									? (points) => callbacks.onRangeChange?.(encoder.id, points)
									: undefined
							}
							onRelease={
								encoder.canRelease && callbacks.onRelease
									? () => callbacks.onRelease?.(encoder.id)
									: undefined
							}
						/>
					) : (
						<HardwareEncoderDisplayView
							key={encoder.id}
							slot={encoder.slot}
							target={
								encoder.target
									? {
											label: encoder.target.label,
											value: encoder.target.display,
											role: encoder.target.role,
										}
									: undefined
							}
							secondary={
								encoder.secondary
									? {
											label: encoder.secondary.label,
											value: encoder.secondary.display,
											role: encoder.secondary.role,
										}
									: undefined
							}
							editValue={normalized(encoder.value) * 100}
							canRelease={encoder.canRelease}
							onEdit={
								encoder.disabled || encoder.indexed || !encoder.target
									? undefined
									: (points) =>
											callbacks.onAbsoluteChange?.(
												encoder.id,
												normalized(points / 100),
											)
							}
							onEditRange={
								callbacks.onRangeChange
									? (points) => callbacks.onRangeChange?.(encoder.id, points)
									: undefined
							}
							onRelease={
								encoder.canRelease && callbacks.onRelease
									? () => callbacks.onRelease?.(encoder.id)
									: undefined
							}
						/>
					),
				)}
			</div>
		</section>
	);
}
