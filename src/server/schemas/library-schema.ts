export type ParameterName = 'dim' | 
    'strobe' | 'iris' | 'prism' | 'effect1' | 'effect2' | 
    'focus' | 'zoom' | 
    'gobo1' | 'gobo1idx' | 'gobo2' | 'gobo2idx' | 
    'red' | 'green' | 'blue' |
    'warm' | 'cold' | 'amber' | 'uv' |
    'color1' | 'color2' | 
    'pan' | 'tilt' | 'movespeed' | 
    'mediapool' | 'mediaidx' | 'mediamode' | 'mediaspeed'

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
                strobe: LibraryFixtureParameterSchema,
                iris: LibraryFixtureParameterSchema,
                prism: LibraryFixtureParameterSchema,
                effect1: LibraryFixtureParameterSchema,
                effect2: LibraryFixtureParameterSchema,
                focus: LibraryFixtureParameterSchema,
                zoom: LibraryFixtureParameterSchema,
                gobo1: LibraryFixtureParameterSchema,
                gobo1idx: LibraryFixtureParameterSchema,
                gobo2: LibraryFixtureParameterSchema,
                gobo2idx: LibraryFixtureParameterSchema,
                red: LibraryFixtureParameterSchema,
                green: LibraryFixtureParameterSchema,
                blue: LibraryFixtureParameterSchema,
                warm: LibraryFixtureParameterSchema,
                cold: LibraryFixtureParameterSchema,
                amber: LibraryFixtureParameterSchema,
                uv: LibraryFixtureParameterSchema,
                color1: LibraryFixtureParameterSchema,
                color2: LibraryFixtureParameterSchema,
                pan: LibraryFixtureParameterSchema,
                tilt: LibraryFixtureParameterSchema,
                movespeed: LibraryFixtureParameterSchema,
                mediapool: LibraryFixtureParameterSchema,
                mediaidx: LibraryFixtureParameterSchema,
                mediamode: LibraryFixtureParameterSchema,
                mediaspeed: LibraryFixtureParameterSchema,
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