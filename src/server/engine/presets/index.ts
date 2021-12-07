import { ValueDimmer,
    ValueColor,
    ValuePos,
    ValueGobo,
    ValueBeam,
    ValueMedia,
    PresetGroupMapping,
    PresetGroup,
} from '../../schemas/show-schema'

export type ValueSource = {} | 'programmer'
export type ValueOrPreset<T extends PresetGroup> = {value: PresetGroupMapping[T], preset?: undefined, source?: ValueSource} | {preset: Preset<T>, value?: undefined, source?: ValueSource}
export type PresetGroupSeting<T> = {
    current: T;
    next?: T;
    delay: number;
    delayStarted?: Date;
    fade: number;
    fadeStarted?: Date;
    changes?: T
}
export type PresetGroupMappingValueOrPreset = {
    'dimmer': PresetGroupSeting<ValueOrPreset<'dimmer'>>;
    'color': PresetGroupSeting<ValueOrPreset<'color'>>;
    'pos': PresetGroupSeting<ValueOrPreset<'pos'>>;
    'gobo': PresetGroupSeting<ValueOrPreset<'gobo'>>;
    'beam': PresetGroupSeting<ValueOrPreset<'beam'>>;
    'media': PresetGroupSeting<ValueOrPreset<'media'>>;
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