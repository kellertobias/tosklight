import type { CSSProperties, ReactNode } from "react";
import type { ModalNumberPresetConfig } from "../input/ModalNumberEditor";
import type { HardwareEncoderDisplayHandle } from "./HardwareEncoderDisplay";
import { HardwareEncoderDisplayView } from "./HardwareEncoderDisplay";
import { TouchEncoder, type TouchEncoderInteraction } from "./TouchEncoder";

export type EncoderSectionSurface = "touch" | "hardware";

export interface EncoderSectionTarget {
	id?: string;
	label: string;
	display: string;
	role?: string;
	value?: number;
}

export interface EncoderSectionItem {
	id: string;
	slot: number;
	target?: EncoderSectionTarget;
	secondary?: EncoderSectionTarget;
	/** Internal encoder value. Defaults preserve the normalized 0–1 domain. */
	value: number;
	minimum?: number;
	maximum?: number;
	inputScale?: number;
	slowStep?: number;
	fastStep?: number;
	repeatSeconds?: number;
	mode?: string;
	accentColor?: string;
	disabled?: boolean;
	indexed?: boolean;
	canRelease?: boolean;
	presets?: ModalNumberPresetConfig;
	touchInteraction?: TouchEncoderInteraction;
}

export interface EncoderSectionModel {
	id: string;
	label: ReactNode;
	description?: ReactNode;
	encoders: readonly EncoderSectionItem[];
}

export interface EncoderSectionCallbacks {
	onRelativeChange?(id: string, delta: number, undoGroup?: string | null): void;
	onAbsoluteChange?(id: string, value: number): void;
	onRangeChange?(id: string, points: number[]): void;
	onRelease?(id: string): void;
	onHardwareDisplayRef?(
		slot: number,
		handle: HardwareEncoderDisplayHandle | null,
	): void;
}

export interface EncoderSectionProps {
	model: EncoderSectionModel;
	surface: EncoderSectionSurface;
	callbacks?: EncoderSectionCallbacks;
	className?: string;
	showHeader?: boolean;
}

function clamped(value: number, minimum = 0, maximum = 1) {
	return Math.max(minimum, Math.min(maximum, value));
}

export function EncoderSection({
	model,
	surface,
	callbacks = {},
	className = "",
	showHeader = true,
}: EncoderSectionProps) {
	return (
		<section
			className={`encoder-section ${surface}-encoder-section ${className}`.trim()}
			aria-label={
				!showHeader && typeof model.label === "string" ? model.label : undefined
			}
			aria-labelledby={showHeader ? `encoder-section-${model.id}` : undefined}
			data-encoder-family={model.id}
			data-encoder-surface={surface}
			style={{
				minWidth: 0,
				minHeight: 0,
				display: "grid",
				gridTemplateRows: showHeader ? "auto minmax(0, 1fr)" : "minmax(0, 1fr)",
			}}
		>
			{showHeader && (
				<header className="encoder-section-header">
					<strong id={`encoder-section-${model.id}`}>{model.label}</strong>
					{model.description != null && <small>{model.description}</small>}
				</header>
			)}
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
				{model.encoders.map((encoder) => (
					<EncoderItem
						key={encoder.id}
						callbacks={callbacks}
						encoder={encoder}
						surface={surface}
					/>
				))}
			</div>
		</section>
	);
}

function EncoderItem({
	encoder,
	surface,
	callbacks,
}: {
	encoder: EncoderSectionItem;
	surface: EncoderSectionSurface;
	callbacks: EncoderSectionCallbacks;
}) {
	if (surface === "touch")
		return (
			<TouchEncoder
				label={`Enc ${encoder.slot} · ${encoder.target?.label ?? "Unassigned"}`}
				slot={encoder.slot}
				attributeLabel={encoder.target?.label ?? "Unassigned"}
				display={encoder.target?.display ?? "—"}
				value={encoder.value}
				minimum={encoder.minimum}
				maximum={encoder.maximum}
				inputScale={encoder.inputScale}
				slowStep={encoder.slowStep}
				fastStep={encoder.fastStep}
				repeatSeconds={encoder.repeatSeconds}
				disabled={encoder.disabled || !encoder.target}
				accentColor={encoder.accentColor}
				mode={encoder.mode}
				indexed={encoder.indexed}
				canRelease={encoder.canRelease}
				presets={encoder.presets}
				touchInteraction={encoder.touchInteraction}
				onStep={(delta, undoGroup) =>
					callbacks.onRelativeChange?.(encoder.id, delta, undoGroup)
				}
				onSet={(value) => callbacks.onAbsoluteChange?.(encoder.id, value)}
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
		);
	return (
		<HardwareEncoderDisplayView
			ref={(handle) => callbacks.onHardwareDisplayRef?.(encoder.slot, handle)}
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
			editValue={encoder.value * (encoder.inputScale ?? 100)}
			secondaryEditValue={
				encoder.secondary?.value !== undefined
					? encoder.secondary.value * (encoder.inputScale ?? 100)
					: undefined
			}
			canRelease={encoder.canRelease}
			presets={encoder.presets}
			onEdit={absoluteHandler(encoder, callbacks)}
			onEditRange={
				callbacks.onRangeChange
					? (points) => callbacks.onRangeChange?.(encoder.id, points)
					: undefined
			}
			onSecondaryEdit={secondaryAbsoluteHandler(encoder, callbacks)}
			onSecondaryEditRange={
				callbacks.onRangeChange && encoder.secondary
					? (points) =>
							callbacks.onRangeChange?.(
								encoder.secondary?.id ?? encoder.id,
								points,
							)
					: undefined
			}
			onRelease={
				encoder.canRelease && callbacks.onRelease
					? () => callbacks.onRelease?.(encoder.id)
					: undefined
			}
		/>
	);
}

function absoluteHandler(
	encoder: EncoderSectionItem,
	callbacks: EncoderSectionCallbacks,
) {
	if (encoder.disabled || encoder.indexed || !encoder.target) return undefined;
	return (points: number) =>
		callbacks.onAbsoluteChange?.(
			encoder.id,
			clamped(
				points / (encoder.inputScale ?? 100),
				encoder.minimum,
				encoder.maximum,
			),
		);
}

function secondaryAbsoluteHandler(
	encoder: EncoderSectionItem,
	callbacks: EncoderSectionCallbacks,
) {
	if (
		encoder.disabled ||
		encoder.indexed ||
		!encoder.secondary ||
		encoder.secondary.value === undefined
	)
		return undefined;
	return (points: number) =>
		callbacks.onAbsoluteChange?.(
			encoder.secondary?.id ?? encoder.id,
			clamped(
				points / (encoder.inputScale ?? 100),
				encoder.minimum,
				encoder.maximum,
			),
		);
}
