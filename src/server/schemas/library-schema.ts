export type ParameterName = 'dimmer' | 
    'beam_strobe' | 'beam_iris' | 'beam_prism' | 'beam_effect' | 
    'frame_1_move' | 'frame_1_swiv' | 'frame_2_move' | 'frame_2_swiv' | 
    'frame_3_move' | 'frame_3_swiv' | 'frame_4_move' | 'frame_5_swiv' | 
    'focus' | 'zoom' | 
    'gobo_1_select' | 'gobo_1_rotate' | 'gobo_2_select' | 'gobo_2_rotate' | 
    'color_red' | 'color_green' | 'color_blue' |
    'color_ww' | 'color_cw' | 'color_amber' | 'color_uv' |
     'color1' | 'color2' | 
     'pos_pan' | 'pos_tilt' | 'pos_speed' | 
     'media_folder' | 'media_file' | 'media_mode' | 'media_play_speed' |
     'misc_1'

export const LibraryFixtureConfigSchema = {
    type: 'object',
    properties: {
        module: {type: 'number', minimum: 1, maximum: 128},
        channel: {type: 'number', minimum: 1, maximum: 512},
        value: {type: 'number', minimum: 0, maximum: 255},
        time: {type: 'string'}
    },
    required: ['module', 'channel', 'value']
} as const

export type LibraryFixtureConfig = {
    module: number;
    channel: number;
    value: number;
    time?: string;
}

export const LibraryFixtureParameterSchema = {
    type: 'object',
    properties: {
        module: {type: 'number', minimum: 1, maximum: 128},
        channel: {type: 'array', items: {type: 'number', minimum: 1, maximum: 512}},
        highlight: {type: 'number', minimum: 0},
        default: {type: 'number', minimum: 0},
        invert: {type: 'boolean'},
        snap: {type: 'boolean'},
        virtual_dimmer: {type: 'boolean'},
        max: {type: 'number', minimum: 0},
        min: {type: 'number', minimum: 0},
    },
    required: ['channel']
} as const


export type LibraryFixtureParameter = {
    channel: number[];
    module?: number;
    highlight?: number;
    default?: number;
    invert?: boolean;
    snap?: boolean;
    virtual_dimmer?: boolean;
    max?: number;
    min?: number;
}

export const LibrarySchema = {
    type: 'object',
    properties: {
        name: {type: 'string', minLength: 2, maxLength: 63},
        short: {type: 'string', minLength: 2, maxLength: 24},
        desc: {type: 'string', minLength: 1, maxLength: 255},
        config: {
            type: 'object',
            properties: {
                startup: LibraryFixtureConfigSchema,
                reset: LibraryFixtureConfigSchema,
                shutdown: LibraryFixtureConfigSchema,
                clear: LibraryFixtureConfigSchema
            }
        },
        parameters: {
            type: 'object',
            properties: {
                dimmer: LibraryFixtureParameterSchema,
                beam_strobe: LibraryFixtureParameterSchema,
                beam_iris: LibraryFixtureParameterSchema,
                beam_prism: LibraryFixtureParameterSchema,
                beam_effect: LibraryFixtureParameterSchema,
                frame_1_move: LibraryFixtureParameterSchema,
                frame_1_swiv: LibraryFixtureParameterSchema,
                frame_2_move: LibraryFixtureParameterSchema,
                frame_2_swiv: LibraryFixtureParameterSchema,
                frame_3_move: LibraryFixtureParameterSchema,
                frame_3_swiv: LibraryFixtureParameterSchema,
                frame_4_move: LibraryFixtureParameterSchema,
                frame_4_swiv: LibraryFixtureParameterSchema,
                focus: LibraryFixtureParameterSchema,
                zoom: LibraryFixtureParameterSchema,
                gobo_1_select: LibraryFixtureParameterSchema,
                gobo_1_rotate: LibraryFixtureParameterSchema,
                gobo_2_select: LibraryFixtureParameterSchema,
                gobo_2_rotate: LibraryFixtureParameterSchema,
                color_red: LibraryFixtureParameterSchema,
                color_green: LibraryFixtureParameterSchema,
                color_blue: LibraryFixtureParameterSchema,
                color_ww: LibraryFixtureParameterSchema,
                color_cw: LibraryFixtureParameterSchema,
                color_amber: LibraryFixtureParameterSchema,
                color_uv: LibraryFixtureParameterSchema,
                color1: LibraryFixtureParameterSchema,
                color2: LibraryFixtureParameterSchema,
                pos_pan: LibraryFixtureParameterSchema,
                pos_tilt: LibraryFixtureParameterSchema,
                pos_speed: LibraryFixtureParameterSchema,
                media_folder: LibraryFixtureParameterSchema,
                media_file: LibraryFixtureParameterSchema,
                media_mode: LibraryFixtureParameterSchema,
                media_speed: LibraryFixtureParameterSchema,
                misc_1: LibraryFixtureParameterSchema,
                misc_2: LibraryFixtureParameterSchema,
                misc_3: LibraryFixtureParameterSchema,
                misc_4: LibraryFixtureParameterSchema,
                misc_5: LibraryFixtureParameterSchema,
                misc_6: LibraryFixtureParameterSchema,
                misc_7: LibraryFixtureParameterSchema,
                misc_8: LibraryFixtureParameterSchema,
            }
        }
    },
    required: ['name', 'parameters']
} as const


export type LibraryEntry = {
    name: string;
    short?: string;
    desc?: string;
    config?: {
        startup?: LibraryFixtureConfig
        reset?: LibraryFixtureConfig
        shutdown?: LibraryFixtureConfig
        clear?: LibraryFixtureConfig
    },
    parameters: Partial<Record<ParameterName, LibraryFixtureParameter>>
}