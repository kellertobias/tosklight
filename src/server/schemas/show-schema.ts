import { ParameterName } from "./library-schema";

export type ValueDimmer = {dim?: number};
export type ValueColor = {red?: number, green?: number, blue?: number, warm?: number, cold?: number, uv?: number, wheel1?: number, wheel2?: number}
export type ValuePos = {pan?: number; tilt?: number, speed?: number, focus?: number}
export type ValueGobo = {gobo1?: number; idx1?: number, gobo2?: number, idx2?: number}
export type ValueBeam = {shut?:number, zoom?: number, effect?: number, iris?: number;}
export type ValueMedia = {pool?: number; idx?: number, mode?: number, speed?: number}


export type PresetGroupMapping = {
    'dimmer': ValueDimmer,
    'color': ValueColor;
    'pos': ValuePos;
    'gobo': ValueGobo;
    'beam': ValueBeam;
    'media': ValueMedia;
}

export const ParamGroupMapping : Record<ParameterName, keyof PresetGroupMapping> = {
    'dim': 'dimmer',
    'beam_strobe': 'beam',
    'beam_iris': 'beam',
    'beam_prism': 'beam',
    'beam_effect': 'beam',
    'frame_1_move': 'beam',
    'frame_1_swiv': 'beam',
    'frame_2_move': 'beam',
    'frame_2_swiv': 'beam',
    'frame_3_move': 'beam',
    'frame_3_swiv': 'beam',
    'frame_4_move': 'beam',
    'focus': 'beam',
    'zoom': 'beam',
    'gobo_1_select': 'gobo',
    'gobo_1_rotate': 'gobo',
    'gobo_2_select': 'gobo',
    'gobo_2_rotate': 'gobo',
    'color_red': 'color',
    'color_green': 'color',
    'color_blue': 'color',
    'color_ww': 'color',
    'color_cw': 'color',
    'color_amber': 'color',
    'color_uv': 'color',
    'color1': 'color',
    'color2': 'color',
    'pos_pan': 'pos',
    'pos_tilt': 'pos',
    'pos_speed': 'pos',
    'media_folder': 'media', 
    'media_file': 'media', 
    'media_mode': 'media', 
    'media_play_speed': 'media', 
}

export const PresetGroupNames = ['dimmer', 'color', 'pos', 'gobo', 'beam', 'media']

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

export type PresetGroup = keyof PresetGroupMapping

export const ShowFixtureSchema = {
    type: 'object',
    properties: {
        type: {type: 'string'},
        name: {type: 'string', minLength: 1},
        patch: {type: 'array', items: {type: ['string', 'number']}}
    }
} as const

export type ShowFixtureType = {
    type: string,
    name: string,
    patch: (string | number)[]
}

export const ShowGroupSchema = {
    type: 'object',
    properties: {
        name: {type: 'string', minLength: 1},
        fixtures: {type: 'array', items: {type: 'string'}}
    }
} as const

export type ShowGroupType = {
    name: string,
    fixtures: string[]
}

export const ShowPresetSchema = {
    type: 'object',
    properties: {
        fixture: {type: 'array', items: {type: 'string'}},
        parameter: {type: 'object'}
    }
} as const

export type ShowPresetType = {
    fixture: string[];
    parameter: Record<string, any>
}

export const ShowPresetGroupSchema = {
    type: 'object',
    patternProperties: {
        '[0-9]+': {type: 'array', items: ShowPresetSchema}
    }
} as const

export type ShowPresetGroupType = Record<string, ShowPresetType[]>

export const ShowSequenceSchema = {
    type: 'object'
} as const


export type ParameterValueConfigStructure = Partial<Record<ParameterName, ValueStructure>>

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

export type ShowSequenceType = {}

export const ShowRoutingSchema = {
    type: 'object',
    properties: {
        dmx: {type: 'string', minLength: 1, maxLength: 1},
        artnet: {
            type: 'object',
            properties: {
                universe: {type: 'number', minimum: 0, maximum: 512},
                node: {type: 'string', format: 'ipv4'}
            },
        },
        size: {type: 'number'}
    },
} as const

export type ShowRoutingType = {
    dmx?: string;
    artnet?: {
        universe: number;
        node: string;
    };
    size?: number;
}

export const ShowSchema = {
    type: 'object',
    properties: {
        name: {type: 'string', minLength: 3, maxLength: 63},
        fixtures: {
            type: 'object',
            patternProperties: {
                '[0-9]+': ShowFixtureSchema
            }
        },
        groups: {
            type: 'object',
            patternProperties: {
                '[0-9]+': ShowGroupSchema
            }
        },
        presets: {
            type: 'object',
            properties: {
                dimmer: ShowPresetGroupSchema,
                color: ShowPresetGroupSchema,
                position: ShowPresetGroupSchema,
            }
        },
        sequences: {
            type: 'object',
            patternProperties: {
                '[0-9]+': ShowSequenceSchema
            }
        },
        routing: {
            type: 'array',
            items: ShowRoutingSchema
        }
    }
} as const

export type ShowType = {
    name: string;
    fixtures: Record<string, ShowFixtureType>;
    groups: Record<string, ShowGroupType>;
    presets: Partial<Record<PresetGroup, ShowPresetGroupType>>;
    sequences: Record<string, ShowSequenceType>;
    routing: ShowRoutingType[]
}
