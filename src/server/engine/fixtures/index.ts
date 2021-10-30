import { getUniverseAndChannel } from "../helpers/patch-notation";
import { FixtureLibrarySrcEntry, FixtureLibrarySrcParam, ParameterName } from "/server/engine/config-reader";
import { DMXChannel, OutputRouting } from "/server/engine/patch/output";

export class Fixture {
    private parameters: Partial<Record<ParameterName, {
        value: number;
        highlight?: number;
        defaultValue: number;
        max: number;
        min: number;
        invert: boolean;
        snap: boolean;
        virtual_dimmer: boolean;
        dmx: DMXChannel[]
    }>> = {}

    constructor(
        fixtureType: FixtureLibrarySrcEntry,
        routing: OutputRouting,
        patch: (number | string)[],
        meta: {name?: string}
    ) {
        const moduleChannels = getUniverseAndChannel(routing, patch)
        Object.entries(fixtureType.parameters).forEach(([param, paramConfig]: [string, FixtureLibrarySrcParam]) => {
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
        })

        this.clear()
    }

    public clear() {
        Object.entries(this.parameters).forEach(([param, {defaultValue}]) => {
            this.setParameter(param, defaultValue)
        })
    }

    public setParameter(name: ParameterName | string, value: number) {
        const param = this.parameters[name]
        if(value < param.min) {
            throw new Error('value-min')
        }
        if(value > param.max) {
            throw new Error('value-max')
        }
        param.value = value;
        for(const channel of param.dmx) {
            channel.value = value % 256
            value = Math.floor(value / 256)
        }
    }

    public highlight
}