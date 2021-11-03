import { Configuration } from "./config-reader";
import { engine } from "./engine";
import { Patch } from './patch'

export class Show {
    patch: Patch;
    constructor() {
        this.patch = new Patch()
    }

    load() {
        engine.stop()
        engine.reset()

        const config = {} as Configuration; // @TODO
        this.patch.setup(config)

        // Register Sequence Tracking first

        // Register Effect Calculation

        // Register DMX Outut Last
        engine.registerTickAction(this.patch.routing.tick)

        engine.start()
    }
}