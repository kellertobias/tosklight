import { ValueDimmer,
    ValueColor,
    ValuePos,
    ValueGobo,
    ValueBeam,
    ValueMedia,
    PresetGroupMapping,
    PresetGroup,
} from '../../schemas/show-schema'

export type ValueOrPreset<T extends PresetGroup> = {value: PresetGroupMapping[T], preset?: undefined} | {preset: Preset<T>, value?: undefined}
export type PresetGroupMappingValueOrPreset = {
    'dimmer': ValueOrPreset<'dimmer'>;
    'color': ValueOrPreset<'color'>;
    'pos': ValueOrPreset<'pos'>;
    'gobo': ValueOrPreset<'gobo'>;
    'beam': ValueOrPreset<'beam'>;
    'media': ValueOrPreset<'media'>;
}



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