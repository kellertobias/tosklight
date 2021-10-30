import { Configuration } from "../config-reader";
import { Fixture } from "../fixtures";
import { OutputRouting } from "./output";

class Patch {
    private routing: OutputRouting;
    private fixtures: Record<string, Fixture>;
    private groups: Record<string, {
        name: string;
        fixtures: Fixture[];
    }>;

    constructor() {}

    getFixture(fixtureId: number | string) {
        const fixture = this.fixtures[`${fixtureId}`]
        if(!fixture) {
            throw new Error('fixture-reference-missing')
        }
        return fixture
    }

    getGroup(groupId: number | string) {
        const group = this.groups[`${groupId}`]
        if(!group) {
            throw new Error('group-reference-missing')
        }
        return group
    }

    setup(config: Configuration) {
        const routing = config.show.routing
        const library = config.library
        this.routing = new OutputRouting(config.show.routing)
        this.fixtures = {}
        
        Object.entries(config.show.fixtures).forEach(([id, fixtConfig]) => {
            const fixtureType = library[fixtConfig.type]
            if (!fixtureType) {
                throw new Error('fixture-type-not-found')
            }
            this.fixtures[id] = new Fixture(
                fixtureType,
                this.routing,
                fixtConfig.patch,
                {...fixtConfig}
            )
        })
        Object.entries(config.show.groups).forEach(([id, {name, fixtures}]) => {
            this.groups[id] = {
                name,
                fixtures: fixtures.map(this.getFixture)
            }
        })
    }
}