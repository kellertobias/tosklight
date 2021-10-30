import { RoutingConfigStructure } from "../config-reader";

export class DMXChannel {
    public readonly universe: number;
    public readonly channel: number;
    private currentValue: number = 0;

    constructor(universe: number, channel: number) {
        this.channel = channel
        this.universe = universe
        this.currentValue = 0
    }

    public set value(value: number) {
        this.currentValue = value
    }

    public get value() {
        return this.currentValue
    }
}

export class OutputUniverse {
    public readonly universe: number;
    public readonly channels: DMXChannel[];

    constructor(universe: number, config: RoutingConfigStructure) {
        this.universe = universe
        this.channels = new Array(config.size ?? 512).map((x, channel) => new DMXChannel(universe, channel))
    }
    
    public sendFrame() {
        console.log(this.channels.map(channel => channel?.value))
    }
}

export class OutputRouting {
    public readonly universes : OutputUniverse[] = []
    constructor(universePatch: RoutingConfigStructure[]) {
        this.universes = universePatch.map((config, unNumber) => {
            return new OutputUniverse(unNumber, config)
        })
    }
}