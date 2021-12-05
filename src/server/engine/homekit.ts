import {Accessory, Characteristic, CharacteristicEventTypes, Service, Categories, uuid} from 'hap-nodejs'

import { Patch } from './patch'

class HomekitControl {
    private patch: Patch | null;

    public load(patch: Patch) {
        this.patch = patch
        const fixtures = this.patch.listFixtures()
        console.log(`[HOMEKIT] Publishing ${fixtures.length} Fixtures`)
        fixtures.forEach((fixture) => {
            console.log(`[HOMEKIT] Publishing ${fixture.fixtureId} - ${fixture.name}`)
            let onState = 0
            const accessory = new Accessory(
                fixture.name,
                uuid.generate(`tosklight.fixtures.${fixture.fixtureId}`)
            )
            const service = new Service.Lightbulb(fixture.name)

            // Dimmer Handling
            if(fixture.hasParameterGroup('dimmer')) {
                const chaPower = service.getCharacteristic(Characteristic.On)
                const chaBright = service.getCharacteristic(Characteristic.Brightness)

                chaPower.on(CharacteristicEventTypes.GET, callback => {
                    callback(undefined, fixture.getParameter('dim'));
                });
                chaPower.on(CharacteristicEventTypes.SET, (valueString, callback) => {
                    try {
                        if(valueString == false) {
                            fixture.setDimmer({value: {dim: 0}}, 'programmer')
                        }
                        callback();
                    } catch(e) {
                        console.log("chaPower.on(CharacteristicEventTypes.SET..", e)
                    }
                });

                chaBright.on(CharacteristicEventTypes.GET, (callback) => {
                    callback(undefined, fixture.getParameter('dim'));
                });

                chaBright.on(CharacteristicEventTypes.SET, (valueString, callback) => {
                    try {
                        console.log("Setting brightness level to:", {valueString});
                        const value = Number(valueString)
                        fixture.setDimmer({value: {dim: value * 255.0 / 100}}, 'programmer')
                        callback();
                    } catch(e) {
                        console.error('Could not set Value for fixture')
                    }
                });
            }

            // Color Handling
            // if(fixture.hasParameterGroup('color')) {
                if(fixture.hasParameter('color_cw') || fixture.hasParameter('color_ww')) {
                    const chaWhiteAdjust = service.getCharacteristic(Characteristic.ColorTemperature)
                }

                const chaHue = service.getCharacteristic(Characteristic.Hue)
                const chaSat = service.getCharacteristic(Characteristic.Saturation)
            // }

            accessory.addService(service)
            accessory.publish({
                username: "17:51:07:F4:BC:8A",
                pincode: '123-45-678',
                category: Categories.LIGHTBULB
            })
        })
    }
}

export const homekit = new HomekitControl()