import {Accessory, Characteristic, CharacteristicEventTypes as CET, Service, Categories, uuid} from 'hap-nodejs'
import ColorConvert from 'color-convert'

import { Patch } from './patch'
import { Fixture } from './fixtures'


const RGBtoHSB = (fixture: Fixture): {hue: number, sat: number, bright: number} => {
    try {
        const red = fixture.getParameter('red') ?? 0
        const green = fixture.getParameter('green') ?? 0
        const blue = fixture.getParameter('blue') ?? 0
        const [hue, sat, bright] = ColorConvert.rgb.hsv(red, green, blue)
    
        return {hue, sat, bright}
    } catch(e) {
        console.log("RGBtoHSB", e)
    }
}

const HSBtoRGB = (hsb: {hue: number, sat: number}, fixture: Fixture): void => {
    const [red, green, blue] = ColorConvert.hsv.rgb([hsb.hue ?? 0, hsb.sat ?? 0, 100])
    fixture.setColor({value: {red, green, blue}}, 'programmer')
}

class HomekitControl {
    private patch: Patch | null;

    public load(patch: Patch) {
        this.patch = patch
        const fixtures = this.patch.listFixtures()
        console.log(`[HOMEKIT] Publishing ${fixtures.length} Fixtures`)
        fixtures.forEach((fixture) => {
            const hex = Number(fixture.fixtureId).toString(16).padStart(6, '0')
            const fixtMac = `${hex[0]}${hex[1]}:${hex[2]}${hex[3]}:${hex[4]}${hex[5]}`
            
            console.log(`[HOMEKIT] Publishing ${fixture.fixtureId} - ${fixture.name} (${fixtMac})`)
            const currentHSB = {hue: 0, sat: 0, bright: 0}
            
            const accessory = new Accessory(
                `ToskLight ${fixture.name ?? `Fixture ${fixture.fixtureId}`}`,
                uuid.generate(`tosklight.fixtures.${fixture.fixtureId}`)
            )
            const service = new Service.Lightbulb(fixture.name)

            // Dimmer Handling
            if(fixture.hasParameterGroup('dimmer')) {
                const chaPower = service.getCharacteristic(Characteristic.On)
                const chaBright = service.getCharacteristic(Characteristic.Brightness)

                chaPower.on(CET.GET, callback => {
                    callback(undefined, fixture.getParameter('dim') > 0);
                });
                chaPower.on(CET.SET, (valueString, callback) => {
                    console.log("Setting on/off level to:", {valueString});
                    if(valueString == false) {
                        fixture.setDimmer({value: {dim: 0}}, 'programmer')
                    } else {
                        fixture.setDimmer({value: {dim: 255}}, 'programmer')
                    }
                    callback();
                });

                chaBright.on(CET.GET, (callback) => {
                    callback(undefined, fixture.getParameter('dim') ?? 0);
                });

                chaBright.on(CET.SET, (valueString, callback) => {
                    console.log("Setting brightness level to:", {valueString});
                    const value = Number(valueString)
                    fixture.setDimmer({value: {dim: value * 255.0 / 100}}, 'programmer')
                    callback();
                });
            }

            // Color Handling
            if(fixture.hasParameterGroup('color')) {
                if(fixture.hasParameter('cold') || fixture.hasParameter('warm')) {
                    const chaWhiteAdjust = service.getCharacteristic(Characteristic.ColorTemperature)
                }

                const chaHue = service.getCharacteristic(Characteristic.Hue)
                chaHue.on(CET.GET, (callback) => {
                    const {hue} = RGBtoHSB(fixture)
                    callback(undefined, hue)
                })
                chaHue.on(CET.SET, (valueString, callback) => {
                    console.log(`Setting Color HUE to ${valueString}`)
                    currentHSB.hue = Number(valueString)
                    HSBtoRGB(currentHSB, fixture)
                    callback()
                })
                const chaSat = service.getCharacteristic(Characteristic.Saturation)
                chaSat.on(CET.GET, (callback) => {
                    const {sat} = RGBtoHSB(fixture)
                    callback(undefined, sat)
                })
                chaSat.on(CET.SET, (valueString, callback) => {
                    console.log(`Setting Color SAT to ${valueString}`)
                    currentHSB.sat = Number(valueString)
                    HSBtoRGB(currentHSB, fixture)
                    callback()
                })
            }

            accessory.addService(service)
            accessory.publish({
                username: `12:34:56:${fixtMac}`,
                pincode: '123-45-678',
                category: Categories.LIGHTBULB
            })
        })
    }
}

export const homekit = new HomekitControl()