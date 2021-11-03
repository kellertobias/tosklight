import { RoutingConfigStructure } from "../config-reader";
import { ArtNetController } from 'artnet-protocol'
import { ArtDmx } from 'artnet-protocol/dist/protocol'

const controller = new ArtNetController()
controller.nameShort = "Tosk"
controller.nameLong = "ToskLight Desk"
controller.bind('0.0.0.0')

export class DMXChannel {
    public readonly universe: number;
    public readonly channel: number;
    private currentValue: number = 0;
    linkedUniverse: DMXUniverse;

    constructor(universe: number, channel: number, linkedUniverse: DMXUniverse) {
        this.channel = channel
        this.universe = universe
        this.currentValue = 0
        this.linkedUniverse = linkedUniverse
    }

    public set value(value: number) {
        this.currentValue = value
        this.linkedUniverse.changed = true
    }

    public get value(): number {
        return this.currentValue
    }

    
}

export class DMXUniverse {
    public readonly universe: number;
    public readonly channels: DMXChannel[];
    public changed = false
    public skipped = 0
    private artnetPacket: ArtDmx

    constructor(universe: number, config: RoutingConfigStructure) {
        this.universe = universe
        
        this.channels = new Array(config.size ?? 512).map((x, channel) => new DMXChannel(universe, channel, this))

        // Reserving memory for the channel values. Reusing the same ArtDMX Packet
        this.artnetPacket = new ArtDmx(0, 0, config.artnet.universe, new Array(config.size ?? 512))
    }
    
    public sendFrame() {
        // More efficient then using array.map
        for(let i = 0; i < this.channels.length; i++) {
            this.artnetPacket.data[i] = this.channels[i].value
        }
        controller.sendBroadcastPacket(this.artnetPacket);
        this.changed = false
    }
}

export class DMXOutput {
    public readonly universes : DMXUniverse[] = []
    constructor(universePatch: RoutingConfigStructure[]) {
        this.universes = universePatch.map((config, unNumber) => {
            return new DMXUniverse(unNumber, config)
        })
    }

    tick(fps: number, time: number) {
        for(let i = 0; i < this.universes.length; i ++) {
            const universe = this.universes[i]
            if(universe.changed || universe.skipped > fps) {
                universe.skipped = 0;
                universe.sendFrame()
            }
            universe.skipped++
        }
    }
}