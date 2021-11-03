import { getUniverseAndChannel } from "../helpers/patch-notation";
import { FixtureLibrarySrcEntry, FixtureLibrarySrcParam, ParameterName } from "/server/engine/config-reader";
import { DMXChannel, DMXOutput } from "/server/engine/patch/output";

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

export class Fixture {
    private parameters: Partial<Record<ParameterName, ParameterConfig>> = {}
    private registeredParameters : ParameterName[] = []
    private highlightParameters : ParameterName[] = []

    constructor(
        fixtureType: FixtureLibrarySrcEntry,
        routing: DMXOutput,
        patch: (number | string)[],
        meta: {name?: string}
    ) {
        const moduleChannels = getUniverseAndChannel(routing, patch)
        Object.entries(fixtureType.parameters).forEach(([param, paramConfig]: [ParameterName, FixtureLibrarySrcParam]) => {
            const {dmxModule, channel} = paramConfig
            this.parameters[param] = {
                dmx: channel.map((singleChannel) => moduleChannels[dmxModule][singleChannel - 1]),
                defaultValue: paramConfig.defaultValue ?? 0,
                value: paramConfig.defaultValue ?? 0,
                max: 256 * channel.length - 1 ?? paramConfig.max,
                min: 0 ?? paramConfig.max,
                invert: false ?? paramConfig.invert,
                snap: false ?? paramConfig.snap,
                virtual_dimmer: paramConfig.virtual_dimmer !== undefined ? paramConfig.virtual_dimmer : (param.startsWith('color_add_') || param.startsWith('color_sub_')),
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
    }

    private setDmxValue(channels: DMXChannel[], value: number) {
        for(const channel of channels) {
            channel.value = value % 256
            value = Math.floor(value / 256)
        }
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
}