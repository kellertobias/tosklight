import { getUniverseAndChannel } from "../helpers/patch-notation";
import { Preset, PresetGroupMappingValueOrPreset, ValueOrPreset } from "../presets";
import { PresetGroup, PresetGroupNames } from "../../schemas/show-schema";

import { DMXChannel, DMXOutput } from "/server/engine/patch/output";
import { LibraryEntry, LibraryFixtureParameter, ParameterName } from "/server/schemas/library-schema";

interface ParameterConfig {
    value: number;
    highlight?: number;
    defaultValue: number;
    max: number;
    min: number;
    invert: boolean;
    snap: boolean;
    virtual_dimmer: boolean;
    dmx: DMXChannel[]
}

type ValueSource = 'programmer' | unknown

export class Fixture {
    private parameters: Partial<Record<ParameterName, ParameterConfig>> = {}
    private registeredParameters : ParameterName[] = []
    private highlightParameters : ParameterName[] = []

    private parameterValues: PresetGroupMappingValueOrPreset

    public readonly fixtureId : string;
    public name : string;

    constructor(
        id: string,
        fixtureType: LibraryEntry,
        routing: DMXOutput,
        patch: (number | string)[],
        meta: {name?: string}
    ) {
        console.log(`[PATCH] Adding Fixture ${fixtureType.name} (${id}/${meta.name}) at ${patch}`)
        this.fixtureId = id;
        this.name = meta.name || `Fixture ${id}`
        const moduleChannels = getUniverseAndChannel(routing, patch)
        if(fixtureType === undefined || fixtureType.parameters === undefined) {
            throw Error('Fixture Type not loaded')
        }
        Object.entries(fixtureType.parameters).forEach(([param, paramConfig]: [ParameterName, LibraryFixtureParameter]) => {
            const {module: dmxModule, channel} = paramConfig

            this.parameters[param] = {
                dmx: channel.map((singleChannel) => moduleChannels[dmxModule - 1][singleChannel - 1]),
                defaultValue: paramConfig.default ?? 0,
                value: paramConfig.default ?? 0,
                max: (256 * channel.length - 1) ?? paramConfig.max,
                min: 0 ?? paramConfig.max,
                invert: false ?? paramConfig.invert,
                snap: false ?? paramConfig.snap,
                virtual_dimmer: paramConfig.virtual_dimmer !== undefined ? paramConfig.virtual_dimmer : (param.startsWith('color_')),
                highlight: paramConfig.highlight,
            }
            this.registeredParameters.push(param)
            if (paramConfig.highlight !== undefined) {
                this.highlightParameters.push(param)
            }
        })
        
        this.clear()
    }

    public clear() {
        //Efficiency Optimization
        for(let i = 0; i < this.registeredParameters.length; i ++) {
            const param = this.registeredParameters[i]
            const { defaultValue } = this.parameters[param]
            this.setParameter(param, defaultValue)
        }
        if(this.parameterValues === undefined) {
            this.parameterValues = {} as PresetGroupMappingValueOrPreset
            for(let i = 0; i < PresetGroupNames.length; i++) {
                const name = PresetGroupNames[i]
                this.parameterValues[name] = {value: {}}
            }
        }
    }

    private setDmxValue(channels: DMXChannel[], value: number) {
        for(const channel of channels) {
            channel.value = value % 256
            value = Math.floor(value / 256)
        }
    }

    public getParameter(parameter: ParameterName): number {
        const param = this.parameters[parameter]
        return param.value / param.max
    }

    public hasParameter(parameter: ParameterName): boolean {
        return this.parameters[parameter] !== undefined
    }

    public hasParameterGroup(pGroup: PresetGroup): boolean {
        return true
    }

    public highlight(enable: boolean = true) {
        //Efficiency Optimization
        for(let i = 0; i < this.highlightParameters.length; i ++) {
            const name = this.highlightParameters[i]
            const { highlight, dmx, value } = this.parameters[name]
            this.setDmxValue(dmx, enable ? highlight : value)
        }
    }

    public setParameter(name: ParameterName | string, value: number) {
        const param : ParameterConfig = this.parameters[name]
        if(value < param.min) {
            throw new Error('value-min')
        }
        if(value > param.max) {
            throw new Error('value-max')
        }
        param.value = value;
        this.setDmxValue(param.dmx, value)
    }

    private setParamGroup<T extends PresetGroup>(groupName: T, data: ValueOrPreset<T>) {
        const lastData = this.parameterValues[groupName]
        this.parameterValues[groupName] = data
        console.log(this.parameterValues)
    }

    // Dimmer Value
    public setDimmer(data: ValueOrPreset<'dimmer'>, source: ValueSource) {
        this.setParamGroup('dimmer', data)
    }

    // Red, Green, Blue || WarmWhite, ColdWhite, UV || Wheel 1, Wheel 2
    public setColor(data: ValueOrPreset<'color'>, source: ValueSource) {
        this.setParamGroup('color', data)
    }

    // Pan, Tile, Speed, Focus
    public setPos(data: ValueOrPreset<'pos'>, source: ValueSource) {
        this.setParamGroup('pos', data)
    }

    // Gobo 1 & Rot., Goto 2 & Rot.
    public setGobo(data: ValueOrPreset<'gobo'>, source: ValueSource) {
        this.setParamGroup('gobo', data)
    }

    // Shutter, Zoom, Iris, Prism/ Effect
    public setBeam(data: ValueOrPreset<'beam'>, source: ValueSource) {
        this.setParamGroup('beam', data)
    }

    // Pool, Index, Mode, Speed
    public setMedia(data: ValueOrPreset<'media'>, source: ValueSource) {
        this.setParamGroup('media', data)
    }

    // Any Command from Fixture Definition
    public sendCommand(command: string) {
        //@TODO
    }
}