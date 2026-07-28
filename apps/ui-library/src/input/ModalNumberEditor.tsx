import {
	type ReactNode,
	type PointerEvent as ReactPointerEvent,
	useState,
} from "react";
import { Button } from "../common/controls/foundation";
import type {
	GroupedSelectionGroup,
	GroupedSelectionOption,
} from "../common/controls/GroupedSelectionField";
import { ModalTitleBar } from "../common/ModalTitleBar";
import { VerticalTouchFaderControl } from "../faders/VerticalTouchFaderControl";
import { ModalLayer } from "../modals/ModalStack";
import { ModalNumberInput, ModalNumberValue } from "./ModalInputControls";
import { UnsavedInputCloseConfirmation } from "./UnsavedInputCloseConfirmation";

export interface ModalNumberFaderConfig {
	label?: string;
	minimum?: number;
	maximum?: number;
	step?: number;
	accentColor?: string;
	valueFromInput?(value: string): number;
	inputFromValue?(value: number): string;
	onChange?(value: number): void;
}

export interface ModalNumberPresetConfig {
	groups: readonly GroupedSelectionGroup<string>[];
	selectedValue?: string;
}

export interface ModalNumberEditorProps {
	ariaLabel: string;
	title: ReactNode;
	value: string;
	onChange(value: string): void;
	onSubmit(value?: string): void;
	onClose(): void;
	allowDecimal?: boolean;
	allowThrough?: boolean;
	replaceOnFirstInput?: boolean;
	dialogClassName?: string;
	beforeTitle?: ReactNode;
	fader?: ModalNumberFaderConfig;
	presets?: ModalNumberPresetConfig;
	presetsOnly?: boolean;
	onPresetSelect?(value: string): void;
	unit?: ReactNode;
	onRelease?(): void;
	releaseLabel?: string;
}

function ModalNumberTitleActions({
	beforeTitle,
	hasPresets,
	mode,
	onModeChange,
	onRelease,
	releaseLabel,
}: {
	beforeTitle?: ReactNode;
	hasPresets: boolean;
	mode: "value" | "presets";
	onModeChange(): void;
	onRelease?: () => void;
	releaseLabel: string;
}) {
	if (!beforeTitle && !hasPresets && !onRelease) return null;
	return (
		<>
			{beforeTitle}
			{hasPresets && (
				<Button
					className="modal-number-mode-toggle"
					aria-label={`Show ${mode === "value" ? "presets" : "value input"}`}
					aria-pressed={mode === "presets"}
					onClick={onModeChange}
				>
					<span data-active={mode === "value" || undefined}>Value</span>
					<span data-active={mode === "presets" || undefined}>Presets</span>
				</Button>
			)}
			{onRelease && (
				<Button
					variant="danger"
					className="modal-number-release"
					onClick={onRelease}
				>
					{releaseLabel}
				</Button>
			)}
		</>
	);
}

export function ModalNumberEditor({
	ariaLabel,
	title,
	value,
	onChange,
	onSubmit,
	onClose,
	allowDecimal = true,
	allowThrough = false,
	replaceOnFirstInput = true,
	dialogClassName = "direct-value-modal",
	beforeTitle,
	fader,
	presets,
	presetsOnly = false,
	onPresetSelect,
	unit,
	onRelease,
	releaseLabel = "Release",
}: ModalNumberEditorProps) {
	const [initialValue] = useState(value);
	const [caret, setCaret] = useState(value.length);
	const [mode, setMode] = useState<"value" | "presets">(
		presetsOnly ? "presets" : "value",
	);
	const [pressedKey, setPressedKey] = useState<string | null>(null);
	const [faderDirty, setFaderDirty] = useState(false);
	const [confirmClose, setConfirmClose] = useState(false);
	const hasPresets = Boolean(presets?.groups.length);
	const hasUnsavedChanges = value !== initialValue || faderDirty;
	const stay = () => setConfirmClose(false);
	const requestClose = () => {
		if (hasUnsavedChanges) {
			setConfirmClose(true);
			return;
		}
		onClose();
	};
	const saveAndClose = () => {
		setConfirmClose(false);
		onSubmit(value);
	};
	const discardAndClose = () => {
		onChange(initialValue);
		if (fader?.onChange) {
			const minimum = fader.minimum ?? 0;
			const maximum = Math.max(minimum, fader.maximum ?? 100);
			const parsed =
				fader.valueFromInput?.(initialValue) ?? Number(initialValue);
			fader.onChange(
				clamp(Number.isFinite(parsed) ? parsed : minimum, minimum, maximum),
			);
		}
		setConfirmClose(false);
		onClose();
	};
	const keypad = (
		<ModalNumberInput
			value={value}
			caret={caret}
			onChange={onChange}
			onCaretChange={setCaret}
			onEnter={() => onSubmit(value)}
			onEscape={requestClose}
			allowDecimal={allowDecimal}
			allowThrough={allowThrough}
			replaceOnFirstInput={replaceOnFirstInput}
			onPressedKeyChange={setPressedKey}
		/>
	);
	return (
		<>
			<ModalLayer
				ariaLabel={ariaLabel}
				dialogClassName={[
					dialogClassName,
					fader && "with-value-fader",
					hasPresets && "with-value-presets",
				]
					.filter(Boolean)
					.join(" ")}
				onClose={requestClose}
			>
				<ModalTitleBar
					title={title}
					actions={
						<ModalNumberTitleActions
							beforeTitle={beforeTitle}
							hasPresets={hasPresets && !presetsOnly}
							mode={mode}
							onModeChange={() =>
								setMode((current) =>
									current === "value" ? "presets" : "value",
								)
							}
							onRelease={onRelease}
							releaseLabel={releaseLabel}
						/>
					}
					closeLabel={`Close ${ariaLabel}`}
					onClose={requestClose}
				/>
				{mode === "presets" && presets ? (
					<ModalNumberPresets
						config={presets}
						onSelect={(next) => {
							if (onPresetSelect) {
								onPresetSelect(next);
								onClose();
								return;
							}
							onChange(next);
							onSubmit(next);
						}}
					/>
				) : (
					<div
						className={`modal-number-editor-content ${fader ? "has-fader" : ""}`}
					>
						{fader && (
							<ModalNumberFader
								ariaLabel={fader.label ?? `${ariaLabel} fader`}
								config={fader}
								inputValue={value}
								onInput={onChange}
								onLiveChange={(next) => {
									setFaderDirty(true);
									fader.onChange?.(next);
								}}
							/>
						)}
						<div className="modal-number-editor-main">
							<ModalNumberValue
								value={value}
								caret={caret}
								onCaretChange={setCaret}
								unit={unit}
								ariaLabel={`${typeof title === "string" ? title : ariaLabel} value`}
								pressedKey={pressedKey}
							/>
							{keypad}
						</div>
					</div>
				)}
			</ModalLayer>
			{confirmClose && (
				<UnsavedInputCloseConfirmation
					ariaLabel={`Unsaved ${ariaLabel} changes`}
					onDiscard={discardAndClose}
					onSave={saveAndClose}
					onStay={stay}
				/>
			)}
		</>
	);
}

function ModalNumberFader({
	ariaLabel,
	config,
	inputValue,
	onInput,
	onLiveChange,
}: {
	ariaLabel: string;
	config: ModalNumberFaderConfig;
	inputValue: string;
	onInput(value: string): void;
	onLiveChange(value: number): void;
}) {
	const minimum = config.minimum ?? 0;
	const maximum = Math.max(minimum, config.maximum ?? 100);
	const parsed = config.valueFromInput?.(inputValue) ?? Number(inputValue);
	const value = clamp(
		Number.isFinite(parsed) ? parsed : minimum,
		minimum,
		maximum,
	);
	const fraction =
		maximum === minimum ? 0 : (value - minimum) / (maximum - minimum);
	const update = (next: number) => {
		const stepped = snapToStep(
			clamp(next, minimum, maximum),
			config.step ?? 0.1,
		);
		onInput(config.inputFromValue?.(stepped) ?? formatNumber(stepped));
		onLiveChange(stepped);
	};
	const updateFromPointer = (event: ReactPointerEvent<HTMLInputElement>) => {
		event.preventDefault();
		const bounds = event.currentTarget.getBoundingClientRect();
		const travel = Math.max(1, bounds.height);
		const offset = clamp(event.clientY - bounds.top, 0, travel);
		const endpointZone = Math.min(32, travel * 0.08);
		if (offset <= endpointZone) {
			update(maximum);
			return;
		}
		if (offset >= travel - endpointZone) {
			update(minimum);
			return;
		}
		const usableTravel = Math.max(1, travel - endpointZone * 2);
		update(
			maximum - ((offset - endpointZone) / usableTravel) * (maximum - minimum),
		);
	};
	return (
		<div className="modal-number-editor-fader vertical-touch-fader-stack">
			<VerticalTouchFaderControl
				label={config.label ?? ariaLabel}
				display={formatNumber(value)}
				fraction={fraction}
				accentColor={config.accentColor}
			>
				<input
					type="range"
					aria-label={ariaLabel}
					min={minimum}
					max={maximum}
					step={config.step ?? 0.1}
					value={value}
					onInput={(event) => update(Number(event.currentTarget.value))}
					onPointerDown={(event) => {
						event.currentTarget.setPointerCapture?.(event.pointerId);
						updateFromPointer(event);
					}}
					onPointerMove={(event) => {
						if (event.currentTarget.hasPointerCapture?.(event.pointerId))
							updateFromPointer(event);
					}}
					onPointerUp={(event) => {
						updateFromPointer(event);
						if (event.currentTarget.hasPointerCapture?.(event.pointerId))
							event.currentTarget.releasePointerCapture(event.pointerId);
					}}
				/>
			</VerticalTouchFaderControl>
		</div>
	);
}

function ModalNumberPresets({
	config,
	onSelect,
}: {
	config: ModalNumberPresetConfig;
	onSelect(value: string): void;
}) {
	return (
		<div className="ui-grouped-selection-groups modal-number-presets">
			{config.groups.map((group) => (
				<section key={group.label}>
					<h3>{group.label}</h3>
					<div className="ui-grouped-selection-options">
						{group.options.map((option) => (
							<PresetButton
								key={option.value}
								option={option}
								selected={option.value === config.selectedValue}
								onSelect={onSelect}
							/>
						))}
					</div>
				</section>
			))}
		</div>
	);
}

function PresetButton({
	option,
	selected,
	onSelect,
}: {
	option: GroupedSelectionOption<string>;
	selected: boolean;
	onSelect(value: string): void;
}) {
	return (
		<Button
			active={selected}
			aria-pressed={selected}
			disabled={option.disabled}
			contentAlign="left"
			onClick={() => onSelect(option.value)}
		>
			<span
				className={`ui-grouped-selection-option ${option.icon ? "has-icon" : "has-no-icon"}`}
			>
				{option.icon && (
					<span className="ui-grouped-selection-icon" aria-hidden="true">
						{option.icon}
					</span>
				)}
				<span className="ui-grouped-selection-copy">
					<b>{option.label}</b>
					{option.description && <small>{option.description}</small>}
				</span>
			</span>
		</Button>
	);
}

function clamp(value: number, minimum: number, maximum: number) {
	return Math.max(minimum, Math.min(maximum, value));
}

function snapToStep(value: number, step: number) {
	if (!Number.isFinite(step) || step <= 0) return value;
	return Math.round(value / step) * step;
}

function formatNumber(value: number) {
	return String(Number(value.toFixed(4)));
}
