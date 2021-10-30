export type ParameterName = 'dimmer' | 'beam_strobe' | 'beam_iris' | 'beam_prism' | 'beam_effect' | 'frame_1_move' | 'frame_1_swiv' | 'frame_2_move' | 'frame_2_swiv' | 'frame_3_move' | 'frame_3_swiv' | 'frame_4_move' | 'frame_5_swiv' | 'focus' | 'zoom' | 'gobo_1_select' | 'gobo_1_rotate' | 'gobo_2_select' | 'gobo_2_rotate' | 'color_add_red' | 'color_add_green' | 'color_add_blue' | 'color_add_ww' | 'color_add_cw' | 'color_add_white' | 'color_add_amber' | 'color_add_uv' | 'color_sub_cyan' | 'color_sub_magenta' | 'color_sub_yellow' | 'color_sub_ctc' | 'color_wheel_a' | 'color_wheel_b' | 'pos_pan' | 'pos_tilt' | 'pos_speed' | 'media_folder' | 'media_file' | 'media_mode' | 'media_play_speed' | 'misc_1'

export type ValueStructure = {
    value?: number;
    delay?: number;
    fade?: number;
    effect?: 'sin' | 'lin' | 'tri' | 'pwm';
    size?: number;
    offset?: number;
    speed?: number;
    duty?: number;
}

export type ParameterValueConfigStructure = Partial<Record<ParameterName, ValueStructure>>

export type FixturePatchConfigStructure = {
    id?: number;
    type: string;
    name?: string;
    short?: string;
    patch: string[];
}

export type GroupConfigStructure = {
    id?: number;
    name?: string;
    fixtures: number[];
}

export type SequenceStepConfigStructure = {
    fixtures: {
        fixture: number;
        parameter: ParameterValueConfigStructure;    
    }[];
    trigger: 'go' | 'follow' | 'time';
    trigTime?: number;
    fadeTime?: number;
    command?: any;
    commandTime?: number;
}

export type SequenceConfigStructure = {
    id?: number;
    name?: string;
    state?: 'on' | 'off';
    atStep?: number;
    steps: SequenceStepConfigStructure[]
}

export type PresetConfigStructure = {
    fixtures: number[] | {
        fixture: number,
        paremeter: ParameterValueConfigStructure
    }[];
    parameter: ParameterValueConfigStructure
}

export type RoutingConfigStructure = {
    dmx?: 'A' | 'B' | 'C' | 'D';
    artnet?: {
        universe: number;
        node: string
    };
    size: number;
}


export type ConfigSourceStructure = {
    name?: string;
    fixtures: Record<string, FixturePatchConfigStructure>;
    groups: Record<string, GroupConfigStructure>;
    sequences: (SequenceConfigStructure | null)[];
    presets: Record<string, (PresetConfigStructure | null)[]>;
    routing: RoutingConfigStructure[];
}

export type FixtureLibrarySrcDmx = {
    dmxModule: number;
    channel: number[];
    defaultValue?: number;
}

export type FixtureLibrarySrcConfig = {
    value: number;
    time: number
} & FixtureLibrarySrcDmx

export type FixtureLibrarySrcParam = {   
    highlight?: number;
    invert?: boolean;
    max?: number;
    min?: number;
    snap?: boolean;
    virtual_dimmer?: boolean;
    mapping?: {
        dmx: number[];
        value?: any;
        min?: number;
        max?: number;
    }[]
} & FixtureLibrarySrcDmx

export type FixtureLibrarySrcEntry = {
    slug: string;
    name: string;
    short?: string;
    desc?:string;
    config: Partial<{
        startup: FixtureLibrarySrcConfig;
        shutdown: FixtureLibrarySrcConfig;
        reset: FixtureLibrarySrcConfig;
        clear: FixtureLibrarySrcConfig;
    }>;
    parameters: Partial<Record<ParameterName, FixtureLibrarySrcParam>> & Record<string, FixtureLibrarySrcDmx>
}

export class Configuration {
    public readonly show: ConfigSourceStructure
    public readonly library: Record<string, FixtureLibrarySrcEntry>
}