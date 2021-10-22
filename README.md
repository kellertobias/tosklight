# ToskLight

The Idea behind this Software is to build a light control software that supports artnet output and a similar structure to industry standard light control software.

As a long time GrandMA 2 user who also supports local theatre clubs I always have the problem that a full fledged GrandMA is too expensive while a onPC solution is too complex to manage for people who are used to work on control desks like the lightcommander.

This is why this software is aimed at this audience.

The goal is to support ArtNET Output on multiple universes and a hardware abstraction layer for controlling multiple different fixtures with the same commands.

The planned basic feature set contains:

-   ArtNET Output on multiple Universes
-   Fixtures patching with Hardware Abstraction (e.g. RGB and CMYK fixtures will be managed the same)
-   Preset Values for Fixtures (e.g. Gobo, Color Wheel Positions)
-   Programmer with command Line input (e.g. `FIXTURE 1 AT 100` or `FIXTURE 1 THRU 10 AT COLOR 0.255.0`)
-   Scenes with Tracking

The goal of this software is to be usable on all systems that are capable of running node.js e.g. also a Raspberry PI. In the beginning, the software will be controllable via a REST-API (running scenes), later it will get a web based user-interface.

## Development

-   `npm run dev` - Client and server are in watch mode with source maps, opens [http://localhost:3000](http://localhost:3000)
-   `npm run build` - `dist` folder will include all the needed files, both client (Bundle) and server.
-   `npm start` - Just runs `node ./dist/server/server.js`
-   `npm start:prod` - sets `NODE_ENV` to `production` and then runs `node ./dist/server/server.js`. (Bypassing webpack proxy)

## Roadmap

-   Basic Software Layout (no UI yet)
-   DMX Output
-   Fixture Basics (Dimmer/Virtual Dimmer, Color, DMX Patch)
-   Read Showfiles from YAML
-   Value-Presets
-   Scenes, Cues (only existing ones from the showfile)
-   Tracking Engine
-   Fadetimes
-   Effects as Values
-   Programmer Matrix and Creating new Scenes in the software

# License

At the moment the License is Proprietary. You can use it for your own purposes but are not allowed to change anything.

It bases on the Boilerplate of https://github.com/gilamran/fullstack-typescript.git
