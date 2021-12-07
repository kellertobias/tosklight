import { getUniverseAndChannel } from "../helpers/patch-notation";
import { Preset, PresetGroupMappingValueOrPreset, ValueOrPreset, ValueSource } from "../presets";
import { ParamGroupMapping, PresetGroup, PresetGroupMapping, PresetGroupNames } from "../../schemas/show-schema";

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

type SetParamOptions = {source?: ValueSource, fade?: number, delay?: number}


export class Fixture {
    public readonly type : LibraryEntry;
    private parameters: Partial<Record<ParameterName, ParameterConfig>> = {}
    private registeredParameters : {param: ParameterName, group: PresetGroup}[] = []
    private highlightParameters : ParameterName[] = []

    private parameterGroups: PresetGroupMappingValueOrPreset = {} as PresetGroupMappingValueOrPreset

    private availableParameterGroups: PresetGroup[] = []

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
        this.type = fixtureType;

        this.name = meta.name || `Fixture ${id}`

        const moduleChannels = getUniverseAndChannel(routing, patch)

        if(fixtureType === undefined || fixtureType.parameters === undefined) {
            throw Error('Fixture Type not loaded')
        }

        let usingVirtualDimmer = false
        const availableParameterGroups : Partial<Record<PresetGroup, true>> = {}

        Object.entries(fixtureType.parameters).forEach(([param, paramConfig]: [ParameterName, LibraryFixtureParameter]) => {
            const {module: dmxModule, channel} = paramConfig
            const group = ParamGroupMapping[param]

            this.parameters[param] = {
                dmx: channel.map((singleChannel) => moduleChannels[(dmxModule ?? 1) - 1][singleChannel - 1]),
                defaultValue: paramConfig.default ?? 0,
                value: paramConfig.default ?? 0,
                max: (256 * channel.length - 1) ?? paramConfig.max,
                min: 0 ?? paramConfig.max,
                invert: false ?? paramConfig.invert,
                snap: false ?? paramConfig.snap,
                virtual_dimmer: paramConfig.virtual_dimmer ?? false,
                highlight: paramConfig.highlight,
            }

            usingVirtualDimmer = usingVirtualDimmer || (paramConfig.virtual_dimmer ?? false)

            this.registeredParameters.push({param, group})
            availableParameterGroups[group] = true

            if (paramConfig.highlight !== undefined) {
                this.highlightParameters.push(param)
            }
        })

        if(fixtureType.parameters.dim === undefined && usingVirtualDimmer) {
            this.parameters.dim = {
                dmx: [],
                defaultValue: 0,
                value: 0,
                max: 255,
                min: 0,
                invert: false,
                snap: false,
                virtual_dimmer: false,
                highlight: 255
            }

            this.highlightParameters.push('dim')
            this.registeredParameters.unshift({param: 'dim', group: 'dimmer'})
            availableParameterGroups['dimmer'] = true
        }

        this.availableParameterGroups = Object.keys(availableParameterGroups) as PresetGroup[]
        
        this.clear()
    }

    public clear() {
        for(let i = 0; i < this.availableParameterGroups.length; i ++) {
            const group = this.availableParameterGroups[i]
            this.parameterGroups[group] = {current: {value: {}}, fade: 0, delay: 0}
        }
        
        //Efficiency Optimization
        for(let i = 0; i < this.registeredParameters.length; i ++) {
            const {param, group} = this.registeredParameters[i]
            const { defaultValue } = this.parameters[param]
            this.setParameter(param, defaultValue)
            this.parameterGroups[group].current.value[param] = defaultValue
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
        if(param === undefined) {
            return undefined
        }
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

    private setParameter(name: ParameterName | string, value: number) {
        const param : ParameterConfig = this.parameters[name]
        if(param === undefined) {
            console.log(name, this.parameters)
        }
        
        param.value = value;
        let dmxValue = value
        if(param.virtual_dimmer) {
            const dimmerValue = this.parameters.dim?.value ?? 100
            dmxValue = dmxValue * dimmerValue / 255.0
        }

        if(dmxValue < param.min) {
            dmxValue = param.min
        }
        if(dmxValue > param.max) {
            dmxValue = param.max
        }

        this.setDmxValue(param.dmx, Math.round(dmxValue))
    }

    private resolvePreset<T extends keyof PresetGroupMappingValueOrPreset>(input: ValueOrPreset<T>): PresetGroupMapping[T] {
        if(input.value !== undefined) {
            return input.value
        }
        throw Error('Presets not yet supported')
    }

    public tick() {
        for(let i = 0; i < this.availableParameterGroups.length; i++) {
            const group = this.availableParameterGroups[i]
            const values = this.resolvePreset(this.parameterGroups[group].current)
            const keys = Object.entries(values)
            for (let j = 0; j < keys.length; j++) {
                const [param, value] = keys[j];
                this.setParameter(param, value)
            }
        }
    }

    // Dimmer Value
    // @TODO: Define SetParamOptions (fade, delay, source)
    // @TODO: Define function to calculate the steps
    // @TODO: If Snap Steps will be set to sero after snapping
    // @TODO: delayStarted and fadeStarted are used to determine if delay/fade is running right now
    //        delay & fade are used for programmer later
    // @TODO: Add delay to fadeStarted start date. If one of these dates is in future, do nothing
    // @TODO: If fadeStarted/ delayStarted is longer then fade/ delay ms ago: set to undefined.
    public setDimmer(data: ValueOrPreset<'dimmer'>, options: SetParamOptions) {
        this.parameterGroups.dimmer = {
            ...this.parameterGroups.dimmer,
            ...options,
            fadeStarted: new Date(),
            delayStarted: new Date(),
            next: data,
        }

        console.log('dimmer', data)
    }

    // Red, Green, Blue || WarmWhite, ColdWhite, UV || Wheel 1, Wheel 2
    public setColor(data: ValueOrPreset<'color'>, options: SetParamOptions) {
        this.parameterGroups.color = {
            ...this.parameterGroups.color,
            ...options,
            fadeStarted: new Date(),
            delayStarted: new Date(),
            next: data,
        }

        console.log('color', data)
    }

    // Pan, Tile, Speed, Focus
    public setPos(data: ValueOrPreset<'pos'>, options: SetParamOptions) {
        this.parameterGroups.pos = {
            ...this.parameterGroups.pos,
            ...options,
            fadeStarted: new Date(),
            delayStarted: new Date(),
            next: data,
        }

    }

    // Gobo 1 & Rot., Goto 2 & Rot.
    public setGobo(data: ValueOrPreset<'gobo'>, options: SetParamOptions) {
        this.parameterGroups.gobo = {
            ...this.parameterGroups.gobo,
            ...options,
            fadeStarted: new Date(),
            delayStarted: new Date(),
            next: data,
        }

    }

    // Shutter, Zoom, Iris, Prism/ Effect
    public setBeam(data: ValueOrPreset<'beam'>, options: SetParamOptions) {
        this.parameterGroups.beam = {
            ...this.parameterGroups.beam,
            ...options,
            fadeStarted: new Date(),
            delayStarted: new Date(),
            next: data,
        }

    }

    // Pool, Index, Mode, Speed
    public setMedia(data: ValueOrPreset<'media'>, options: SetParamOptions) {
        this.parameterGroups.media = {
            ...this.parameterGroups.media,
            ...options,
            fadeStarted: new Date(),
            delayStarted: new Date(),
            next: data,
        }

    }

    // Any Command from Fixture Definition
    public sendCommand(command: string) {
        //@TODO
    }
}