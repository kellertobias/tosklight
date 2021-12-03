import { LibraryEntry } from "../schemas/library-schema"
import { ShowType } from "../schemas/show-schema"

export class Configuration {
    public readonly show: ShowType
    public readonly library: Record<string, LibraryEntry>
}