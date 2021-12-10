import { DMXChannel, DMXOutput } from "/server/engine/patch/output"

export const getUniverseAndChannel = (routing: DMXOutput, patch: (number | string)[]): Record<string, DMXChannel[]> => {
    const patchChannels: Record<string, DMXChannel[]> = {}
    patch.forEach((channelNumOrString, mod) => {
        if (typeof(channelNumOrString) === 'number' || !channelNumOrString.includes('.')) {
            let channel = Number.parseInt(`${channelNumOrString}`, 10) - 1

            for (const universe of routing.universes) {
                if (channel > universe.channels.length) {
                    channel = channel - universe.channels.length
                }
                if (channel < universe.channels.length) {
                    patchChannels[mod] = universe.channels.slice(channel)
                    return
                }
            }
            throw new Error('patch')
        }
        const [univNumStr, channelStr] = channelNumOrString.split('.')
        const univNum = Number.parseInt(univNumStr, 10) - 1
        const channel = Number.parseInt(channelStr, 10) - 1

        const universe = routing.universes[univNum]
        if(!universe) {
            throw new Error('patch')
        }

        if(channel >= universe.channels.length) {
            throw new Error('patch')
        }

        patchChannels[mod] = universe.channels.slice(channel)
    })
    console.log(patchChannels['0'][0]?.channel, patchChannels['0'][0]?.linkedUniverse.universe)
    return patchChannels
}
