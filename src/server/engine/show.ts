import fs from 'fs'
import yaml from 'js-yaml'
import AJV, { ErrorObject } from 'ajv'
import addFormats from "ajv-formats"

import { LibrarySchema, LibraryEntry } from '../schemas/library-schema';
import { ShowSchema, ShowType } from '../schemas/show-schema';

import { Configuration } from "./config-reader";
import { engine } from "./engine";
import { Patch } from './patch'
import { homekit } from "./homekit";

const ajv = new AJV({
    allowUnionTypes: true
})
addFormats(ajv)

const validateLibrary = ajv.compile(LibrarySchema)
const validateShow = ajv.compile(ShowSchema)

const printSchemaErrors = (filename: string, data: unknown, errors: ErrorObject[]) => {
    console.log(`Validation of ${filename} failed:`)
    errors.forEach(error => {
        const key = error.instancePath.split('/').join('.');
        console.log(` - ${error.keyword}: ${key} - ${JSON.stringify(error.params)}`)
    })
    console.log('')
}

export class Show {
    library: Record<string, LibraryEntry>
    patch: Patch;
    constructor() {
        this.patch = new Patch()
    }

    private loadLibrary(): {errors: string[], library: Record<string, LibraryEntry>} {
        const path = './data/library/'
        console.log(`Loading Library ${path}`)
        const files = fs.readdirSync(path).filter(
            file => fs.lstatSync(`${path}${file}`).isFile()
        ).filter(
            file => !file.startsWith('_')
        )

        const library : Record<string, LibraryEntry> = {}
        const errors : string[] = []
        files.forEach((file) => {
            console.log(` - ${file}`)
            const libraryEntry = yaml.load(fs.readFileSync(`${path}${file}`, 'utf8')) as LibraryEntry
            const name = file.split('.')[0]
            const entryValid = validateLibrary(libraryEntry)
            if(entryValid) {
                library[name] = libraryEntry
            } else {
                printSchemaErrors(file, libraryEntry, validateLibrary.errors)
                errors.push(file)
            }
        })
        console.log(` ${Object.entries(library).length} Fixture Types Loaded...\n`)
        return {errors, library}
    }

    private loadConfig(filename: string): ShowType | undefined {
        console.log(`Loading Show ${filename}`)
        const show = yaml.load(fs.readFileSync(filename), 'utf8') as ShowType
        const valid = validateShow(show)
        if(valid) {
            return show
        }
        printSchemaErrors(filename, show, validateShow.errors)
    }

    load() {
        engine.stop()
        engine.reset()

        const {errors, library} = this.loadLibrary()
        const show = this.loadConfig('./data/shows/minimal.yaml')
        if(!show) {
            console.log(`Showfile Empty or not Correct. Aborting`)
            return
        }

        this.patch.setup({library, show})

        // Register Sequence Tracking first

        // Register Effect Calculation

        // Register DMX Outut Last
        engine.registerTickAction(this.patch.routing.tick)

        homekit.load(this.patch)
        engine.start()
    }
}

export const show = new Show()