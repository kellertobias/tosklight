import type {
	CSSProperties,
	MouseEventHandler,
	PointerEventHandler,
	ReactNode,
} from "react";
import { Button, Input } from "../controls";
import {
	type VerticalTouchFaderAction,
	VerticalTouchFaderSurface,
} from "../faders";

export interface PlaybackCardViewModel {
	page: number;
	slot: number;
	row: number;
	rowUnits: number;
	name: string;
	assigned: boolean;
	selected?: boolean;
	selectionPending?: boolean;
	className?: string;
	color?: string;
	hasFader: boolean;
	faderValue: number;
	faderLabel: string;
	faderDisplay: string;
	faderMode?: string;
	status?: { kind: "loaded" | "flash" | "swap"; label: string };
	hardwarePickup?: {
		physicalPosition: number;
		pickupTarget: number;
	};
	disabled?: boolean;
	actions: VerticalTouchFaderAction[];
	assignment?: { label: ReactNode; detail: ReactNode };
	configuration?: { label: ReactNode; detail: ReactNode };
}

export interface PlaybackCardCallbacks {
	onActivate?: MouseEventHandler<HTMLElement>;
	onPointerDownCapture?: PointerEventHandler<HTMLElement>;
	onClickCapture?: MouseEventHandler<HTMLElement>;
	onFaderChange?: (value: number) => void;
}

export interface PlaybackBankItem {
	model: PlaybackCardViewModel;
	callbacks?: PlaybackCardCallbacks;
	cueRows?: ReactNode;
	group?: { name: string; master: string };
}

export interface PlaybackBankViewProps {
	mode: "touch" | "hardware";
	items: readonly PlaybackBankItem[];
	className?: string;
}

function CardOverlays({ model }: { model: PlaybackCardViewModel }) {
	return (
		<>
			{model.assignment && (
				<Button className="playback-assignment-target">
					<b>{model.assignment.label}</b>
					<small>{model.assignment.detail}</small>
				</Button>
			)}
			{model.configuration && (
				<div
					className="playback-assignment-target playback-configuration-target"
					aria-hidden="true"
				>
					<b>{model.configuration.label}</b>
					<small>{model.configuration.detail}</small>
				</div>
			)}
		</>
	);
}

function PlaybackActionButtons({
	actions,
}: {
	actions: VerticalTouchFaderAction[];
}) {
	return actions.map(({ id, label, ...props }) => (
		<Button {...props} key={id}>
			{label}
		</Button>
	));
}

function PlaybackStatus({ model }: { model: PlaybackCardViewModel }) {
	if (!model.status) return null;
	return (
		<span
			className={`playback-status playback-status-${model.status.kind}`}
			role="status"
		>
			{model.status.label}
		</span>
	);
}

function cardData(model: PlaybackCardViewModel) {
	return {
		"data-page": model.page,
		"data-playback-slot": model.slot,
		"data-playback-row": model.row,
		"data-row-units": model.rowUnits,
		"data-selected-playback": model.selected || undefined,
		"data-selection-pending": model.selectionPending || undefined,
	};
}

export function TouchPlaybackCardView({
	model,
	callbacks = {},
}: {
	model: PlaybackCardViewModel;
	callbacks?: PlaybackCardCallbacks;
}) {
	return (
		// biome-ignore lint/a11y/useKeyWithClickEvents: The article delegates keyboard interaction to its real child controls.
		<article
			{...cardData(model)}
			data-set-click-target
			className={model.className}
			style={
				model.color
					? ({ "--playback-color": model.color } as CSSProperties)
					: undefined
			}
			onPointerDownCapture={callbacks.onPointerDownCapture}
			onClickCapture={callbacks.onClickCapture}
			onClick={callbacks.onActivate}
		>
			<CardOverlays model={model} />
			<Button
				className="playback-software-representation"
				aria-label={`Playback representation page ${model.page} playback ${model.slot}`}
			>
				<b>
					{model.slot} · {model.name}
				</b>
			</Button>
			<PlaybackStatus model={model} />
			{model.hasFader && (
				<VerticalTouchFaderSurface
					hardware={false}
					disabled={model.disabled || !model.assigned}
					label={model.faderLabel}
					value={model.faderValue}
					accentColor={model.color}
					mode={model.faderMode}
					display={model.faderDisplay}
					actions={model.actions}
					onChange={callbacks.onFaderChange}
				/>
			)}
			{!model.hasFader && model.actions.length > 0 && (
				<footer
					className={`faderless-playback-actions action-count-${model.actions.length}`}
					style={
						{ "--playback-action-count": model.actions.length } as CSSProperties
					}
				>
					<PlaybackActionButtons actions={model.actions} />
				</footer>
			)}
		</article>
	);
}

export interface HardwareCueView {
	number: number;
	name?: string;
	fadeMillis?: number;
}

export function HardwareCueRowsView({
	previous,
	current,
	next,
	compact = false,
	nextLoaded = false,
	progress = 0,
}: {
	previous?: HardwareCueView;
	current?: HardwareCueView;
	next?: HardwareCueView;
	compact?: boolean;
	nextLoaded?: boolean;
	progress?: number;
}) {
	const rows = compact
		? ([
				[nextLoaded ? next : current, nextLoaded ? "next" : "current"],
			] as const)
		: ([
				[previous, "previous"],
				[current, "current"],
				[next, "next"],
			] as const);
	return (
		<div className={`hardware-cue-list ${compact ? "single" : "triple"}`}>
			{rows.map(([cue, kind]) => (
				<div
					className={`hardware-cue-row ${kind} ${kind === "next" && nextLoaded ? "loaded-next" : ""}`}
					style={
						kind === "current"
							? ({ "--cue-fade-progress": progress } as CSSProperties)
							: undefined
					}
					key={kind}
				>
					<span>{cue?.number ?? "—"}</span>
					<b>{cue?.name || (cue ? `Cue ${cue.number}` : "—")}</b>
					{(kind === "next" && nextLoaded) || cue?.fadeMillis ? (
						<small>
							{kind === "next" && nextLoaded
								? "LOADED NEXT"
								: `${((cue?.fadeMillis ?? 0) / 1000).toFixed(1)}s`}
						</small>
					) : null}
				</div>
			))}
		</div>
	);
}

export function HardwarePlaybackCardView({
	model,
	cueRows,
	group,
	callbacks = {},
}: {
	model: PlaybackCardViewModel;
	cueRows?: ReactNode;
	group?: { name: string; master: string };
	callbacks?: PlaybackCardCallbacks;
}) {
	return (
		// biome-ignore lint/a11y/useKeyWithClickEvents: The article delegates keyboard interaction to its real child controls.
		<article
			{...cardData(model)}
			data-set-click-target
			className={`hardware-playback-card ${model.className ?? ""}`}
			style={
				model.color
					? ({ "--playback-color": model.color } as CSSProperties)
					: undefined
			}
			onPointerDownCapture={callbacks.onPointerDownCapture}
			onClickCapture={callbacks.onClickCapture}
			onClick={callbacks.onActivate}
		>
			<CardOverlays model={model} />
			<header>
				<div className="playback-software-representation">
					<b>
						{model.slot} · {model.name}
					</b>
				</div>
				<strong>
					{model.page}.{model.slot}
				</strong>
			</header>
			<PlaybackStatus model={model} />
			{cueRows ??
				(group ? (
					<div className="hardware-cue-list single">
						<div className="hardware-cue-row current">
							<span>GRP</span>
							<b>{group.name}</b>
							<small>{group.master} master</small>
						</div>
					</div>
				) : (
					<div className="hardware-cue-list single" />
				))}
			<div className="hardware-playback-controls">
				<footer>
					<PlaybackActionButtons actions={model.actions} />
				</footer>
				{model.hasFader && (
					<HardwarePlaybackFaderView
						ariaLabel={`Page ${model.page} playback ${model.slot} fader`}
						disabled={model.disabled || !model.assigned}
						display={model.faderDisplay}
						value={model.faderValue}
						pickup={model.hardwarePickup}
						onChange={callbacks.onFaderChange}
					/>
				)}
			</div>
		</article>
	);
}

export interface HardwarePickupPresentation {
	/** Latest real non-motorized hardware position, normalized to 0–1. */
	physicalPosition: number;
	/** Authoritative position that releases pickup, normalized to 0–1. */
	pickupTarget: number;
}

export interface HardwarePickupGeometry {
	physicalPercent: number;
	targetPercent: number;
	segmentStartPercent: number;
	segmentSizePercent: number;
	direction: "raise" | "lower" | "satisfied";
}

const PICKUP_RENDER_EPSILON = 0.000_1;

export function hardwarePickupGeometry(
	pickup: HardwarePickupPresentation,
): HardwarePickupGeometry {
	const physical = Math.min(1, Math.max(0, pickup.physicalPosition));
	const target = Math.min(1, Math.max(0, pickup.pickupTarget));
	const difference = target - physical;
	return {
		physicalPercent: physical * 100,
		targetPercent: target * 100,
		segmentStartPercent: Math.min(physical, target) * 100,
		segmentSizePercent:
			Math.abs(difference) <= PICKUP_RENDER_EPSILON
				? 0
				: Math.abs(difference) * 100,
		direction:
			Math.abs(difference) <= PICKUP_RENDER_EPSILON
				? "satisfied"
				: difference > 0
					? "raise"
					: "lower",
	};
}

export function HardwarePlaybackFaderView({
	ariaLabel,
	disabled,
	display,
	value,
	pickup,
	onChange,
}: {
	ariaLabel: string;
	disabled?: boolean;
	display: string;
	value: number;
	pickup?: HardwarePickupPresentation;
	onChange?: (value: number) => void;
}) {
	const geometry = pickup ? hardwarePickupGeometry(pickup) : null;
	const pickupVisible = Boolean(geometry && geometry.segmentSizePercent > 0);
	const physicalLabel = geometry
		? `${Math.round(geometry.physicalPercent)}%`
		: display;
	const targetLabel = geometry
		? `${Math.round(geometry.targetPercent)}%`
		: null;
	return (
		// biome-ignore lint/a11y/noLabelWithoutControl: Input renders the native range inside this label.
		<label
			className={`hardware-fader ${pickupVisible ? "pickup-required" : ""}`}
			data-pickup-direction={pickupVisible ? geometry?.direction : undefined}
			data-pickup-physical={pickup?.physicalPosition}
			data-pickup-target={pickup?.pickupTarget}
			style={
				{
					"--hardware-fader-level": `${geometry?.physicalPercent ?? value}%`,
					"--hardware-pickup-start": `${geometry?.segmentStartPercent ?? 0}%`,
					"--hardware-pickup-size": `${geometry?.segmentSizePercent ?? 0}%`,
					"--hardware-pickup-target": `${geometry?.targetPercent ?? 0}%`,
				} as CSSProperties
			}
		>
			<i className="hardware-fader-fill" />
			{pickupVisible && (
				<>
					<i className="hardware-fader-pickup-difference" />
					<i className="hardware-fader-physical-marker" />
					<i className="hardware-fader-target-marker" />
				</>
			)}
			<b>
				{pickupVisible && targetLabel ? (
					<>
						<span>
							Physical {physicalLabel} · Target {targetLabel}
						</span>
						<small>
							{geometry?.direction === "raise" ? "Raise" : "Lower"} to{" "}
							{targetLabel}
						</small>
					</>
				) : (
					display
				)}
			</b>
			<Input
				aria-label={ariaLabel}
				aria-description={
					pickupVisible && targetLabel
						? `Physical ${physicalLabel}. Target ${targetLabel}. ${geometry?.direction === "raise" ? "Raise" : "Lower"} to ${targetLabel}.`
						: undefined
				}
				type="range"
				min="0"
				max="100"
				step="0.1"
				value={value}
				disabled={disabled}
				onInput={(event) => onChange?.(Number(event.currentTarget.value))}
			/>
		</label>
	);
}

export function PlaybackBankView({
	mode,
	items,
	className = "",
}: PlaybackBankViewProps) {
	const bank = (
		<div
			className={`playback-fader-bank ${mode}-layout ${className}`.trim()}
			data-playback-bank-mode={mode}
		>
			{items.map((item) =>
				mode === "touch" ? (
					<TouchPlaybackCardView
						key={`${item.model.page}-${item.model.slot}`}
						model={item.model}
						callbacks={item.callbacks}
					/>
				) : (
					<HardwarePlaybackCardView
						key={`${item.model.page}-${item.model.slot}`}
						model={item.model}
						callbacks={item.callbacks}
						cueRows={item.cueRows}
						group={item.group}
					/>
				),
			)}
		</div>
	);
	return mode === "hardware" ? (
		<div className="hardware-connected">{bank}</div>
	) : (
		bank
	);
}
