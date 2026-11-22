import { NumberField, SelectField } from "@tosklight/ui/controls";
import type { AuxiliaryLayer, LayerPreset } from "../../shared/api/automation";
import type { CatalogView } from "../../shared/api/generated/media-wire";
import { useEffects, useModels } from "../../shared/api/queries";
import { AssetPicker } from "./AutomationShared";

const BLENDS = ["normal", "add", "screen", "multiply", "overlay", "difference", "lighten", "darken"];
const SCALE_MODES = ["fit", "fill", "original", "stretch"];

export function AuxiliaryLayerEditor({ value, catalog, onChange }: { value: AuxiliaryLayer; catalog: CatalogView | undefined; onChange: (next: AuxiliaryLayer) => void }) {
	const effects = useEffects();
	const models = useModels();
	const controls = value.controls;
	const change = (next: Partial<AuxiliaryLayer>) => onChange({ ...value, ...next });
	const control = (next: Partial<LayerPreset>) => change({ controls: { ...controls, ...next } });
	const number = (label: string, field: keyof LayerPreset, fallback: number, min: number, max: number, step = 0.01) => <NumberField key={String(field)} label={label} value={Number(controls[field] ?? fallback)} min={min} max={max} step={step} onChange={(event) => { const next = Number(event.target.value); if (Number.isFinite(next)) control({ [field]: next }); }} />;
	const tint = controls.tint ?? { red: 1, green: 1, blue: 1 };
	const mask = controls.mask ?? { address: { folder: 0, file: 0 }, scaleX: 1, scaleY: 1, positionX: 0, positionY: 0, invert: false, opacity: 0, source: "luminance" };
	return <div className="pixel-auxiliary-layer" aria-label={`Layer ${value.layer} settings`}>
		<div className="pixel-auxiliary-choice"><strong>Layer {value.layer}</strong><SelectField label={`Layer ${value.layer} behavior`} ariaLabel={`Layer ${value.layer} behavior`} value={value.mode} options={[
			{ value: "unchanged", label: "Leave unchanged" }, { value: "off", label: "Off" }, { value: "fixed", label: "Fixed content" },
		]} onChange={(mode) => change({ mode })} /></div>
		{value.mode === "fixed" && <>
			<AssetPicker label={`Layer ${value.layer} content`} catalog={catalog} assetId={value.assetId} onChange={(assetId) => change({ assetId })} />
			<div className="pixel-form-grid">
				<SelectField label="Play mode" ariaLabel={`Layer ${value.layer} play mode`} value={typeof controls.playMode === "string" ? controls.playMode : "loop"} options={["loop", "reverse", "bounce", "pause", "stop"].map((mode) => ({ value: mode, label: mode[0].toUpperCase() + mode.slice(1) }))} onChange={(playMode) => control({ playMode })} />
				{number("Dimmer", "dimmer", 1, 0, 1)}{number("Volume", "volume", 1, 0, 1)}
			</div>
			<details><summary>Frame, colour, mask and effects</summary><div className="pixel-form-grid">
				{number("Scale X", "scaleX", 1, -4, 4)}{number("Scale Y", "scaleY", 1, -4, 4)}
				<SelectField label="Scaling mode" ariaLabel={`Layer ${value.layer} scaling mode`} value={controls.scalingMode ?? "fit"} options={SCALE_MODES.map((mode) => ({ value: mode, label: mode[0].toUpperCase() + mode.slice(1) }))} onChange={(scalingMode) => control({ scalingMode })} />
				{number("Position X", "positionX", 0, -4, 4)}{number("Position Y", "positionY", 0, -4, 4)}{number("Rotation (°)", "rotation", 0, -360, 360, 1)}
				{(["red", "green", "blue"] as const).map((channel) => <NumberField key={channel} label={`Tint ${channel}`} value={tint[channel]} min={0} max={1} step={0.01} onChange={(event) => control({ tint: { ...tint, [channel]: Number(event.target.value) } })} />)}
				{number("Greyscale", "grayscale", 0, 0, 1)}
				<SelectField label="Blend" ariaLabel={`Layer ${value.layer} blend`} value={controls.blend ?? "normal"} options={BLENDS.map((mode) => ({ value: mode, label: mode[0].toUpperCase() + mode.slice(1) }))} onChange={(blend) => control({ blend })} />
				{number("In point (frames)", "inPoint", 0, 0, 65535, 1)}{number("Out point (frames)", "outPoint", 0, 0, 65535, 1)}
				<SelectField label="3D model" ariaLabel={`Layer ${value.layer} 3D model`} value={String(controls.model?.model ?? 0)} options={[{ value: "0", label: "Flat" }, ...(models.data ?? []).map((model) => ({ value: String(model.slot), label: `${model.slot} · ${model.name}` }))]} onChange={(model) => control({ model: { model: Number(model), pan: controls.model?.pan ?? 0, tilt: controls.model?.tilt ?? 0 } })} />
				<NumberField label="Model pan (°)" value={controls.model?.pan ?? 0} step={1} onChange={(event) => control({ model: { model: controls.model?.model ?? 0, pan: Number(event.target.value), tilt: controls.model?.tilt ?? 0 } })} />
				<NumberField label="Model tilt (°)" value={controls.model?.tilt ?? 0} step={1} onChange={(event) => control({ model: { model: controls.model?.model ?? 0, pan: controls.model?.pan ?? 0, tilt: Number(event.target.value) } })} />
			</div>
			<div className="pixel-form-grid">
				<SelectField label="Mask" ariaLabel={`Layer ${value.layer} mask`} value={`${mask.address.folder}/${mask.address.file}`} options={[{ value: "0/0", label: "No mask" }, ...(catalog?.folders ?? []).flatMap((folder) => folder.items.map((item) => ({ value: `${folder.folder}/${item.file}`, label: `${folder.folder}/${item.file} · ${item.name}` })))]} onChange={(address) => { const [folder, file] = address.split("/").map(Number); control({ mask: { ...mask, address: { folder, file }, opacity: folder ? 1 : 0 } }); }} />
				<NumberField label="Mask opacity" value={mask.opacity} min={0} max={1} step={0.01} onChange={(event) => control({ mask: { ...mask, opacity: Number(event.target.value) } })} />
				{([0, 1] as const).map((index) => {
					const banks = controls.effectBanks ?? [{ select: 0, strength: 0, parameters: [0, 0, 0, 0] }, { select: 0, strength: 0, parameters: [0, 0, 0, 0] }];
					const bank = banks[index];
					const updateBank = (next: Partial<typeof bank>) => control({ effectBanks: banks.map((current, position) => position === index ? { ...current, ...next } : current) });
					return <div key={index} className="pixel-effect-bank"><SelectField label={`Effect bank ${index + 1}`} ariaLabel={`Layer ${value.layer} effect bank ${index + 1}`} value={String(bank.select)} options={[{ value: "0", label: "Off" }, ...(effects.data ?? []).map((effect) => ({ value: String(effect.slot), label: `${effect.slot} · ${effect.name}` }))]} onChange={(select) => updateBank({ select: Number(select), strength: select === "0" ? 0 : 1 })} />
						<NumberField label={`Bank ${index + 1} strength`} value={bank.strength} min={0} max={1} step={0.01} onChange={(event) => updateBank({ strength: Number(event.target.value) })} />
					</div>;
				})}
			</div></details>
		</>}
	</div>;
}
