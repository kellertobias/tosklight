import { ValueDimmer,
    ValueColor,
    ValuePos,
    ValueGobo,
    ValueBeam,
    ValueMedia,
    PresetGroupMapping,
    PresetGroup,
} from '../../schemas/show-schema'

export type ValueOrPreset<T extends PresetGroup> = {value: PresetGroupMapping[T]} | {preset: Preset<T>}
export type PresetGroupMappingValueOrPreset = Record<PresetGroup, ValueOrPreset<PresetGroup>>


export class Preset<T extends PresetGroup> {
    public name: string;
    public type: T;
    public value: PresetGroupMapping[T];

    constructor(name: string, value: PresetGroupMapping[T]) {
        this.name = name;
        this.value = value;
    }
}

export class PresetPool {
    public presets = {}
}