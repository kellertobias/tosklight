import { Configuration } from "../config-reader";
import { Fixture } from "../fixtures";
import { DMXOutput } from "./output";

export class Patch {
    public routing: DMXOutput;
    private fixtures: Record<string, Fixture> = {};
    private fixtureList : Fixture[] = []
    private groups: Record<string, {
        name: string;
        fixtures: Fixture[];
    }> = {};

    constructor() {}

    listFixtures = (): Fixture[] => {
        return Object.values(this.fixtures)
    }

    getFixture = (fixtureId: number | string) => {
        const fixture = this.fixtures[`${fixtureId}`]
        if(!fixture) {
            throw new Error('fixture-reference-missing')
        }
        return fixture
    }

    getGroup = (groupId: number | string) => {
        const group = this.groups[`${groupId}`]
        if(!group) {
            throw new Error('group-reference-missing')
        }
        return group
    }

    tick = () => {
        // For loop instead of list.forEach for performance reasons
        for(let i = 0; i < this.fixtureList.length; i++) {
            const fixture = this.fixtureList[i];
            fixture.tick()
        }
    }

    setup = (config: Configuration) => {
        const routing = config.show.routing
        const library = config.library
        this.routing = new DMXOutput(config.show.routing)
        this.fixtures = {}
        
        Object.entries(config.show.fixtures).forEach(([id, fixtConfig]) => {
            const fixtureType = library[fixtConfig.type]
            if (!fixtureType) {
                throw new Error('fixture-type-not-found')
            }
            this.fixtures[id] = new Fixture(
                id,
                fixtureType,
                this.routing,
                fixtConfig.patch,
                {...fixtConfig}
            )
        })

        this.fixtureList = Object.values(this.fixtures)

        Object.entries(config?.show?.groups ?? {}).forEach(([id, {name, fixtures}]) => {
            this.groups[id] = {
                name,
                fixtures: fixtures.map(this.getFixture)
            }
        })
    }
}