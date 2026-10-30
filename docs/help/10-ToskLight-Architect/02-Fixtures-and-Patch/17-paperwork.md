# Lighting paperwork

The patch sheet builds the paperwork a crew hangs, patches and focuses from. It always reads the current patch and positions, so it is up to date whenever you open it.

Choose **Paperwork** in the patch sheet header. It is the same in Architect and in Control (**Show > Show Patch**). Choose a report in **Report**:

| Report | Lists | Order |
| --- | --- | --- |
| **Channel Hookup** | Every instrument: fixture ID, name, type, mode, DMX address, position, unit and colour. | Fixture ID |
| **Instrument Schedule** | The same instruments by hanging position: position, unit, fixture, type, mode, DMX, colour and accessories. | Position, then unit |
| **DMX Sheet** | Every patched split: universe, start and end address, fixture, type, mode and split. | Universe, then address |
| **Color Pull List** | Each gel with its count and fixtures. Open white is left out. | Colour |
| **Accessory Pull List** | Shaper modules, manual lenses and shutter cuts, with counts and fixtures. | Accessory |
| **Symbol Key** | Each fixture type with its count, how many are unpatched, and its modes. | Type |

A fixture's position is the 3D Point it follows as its Position Reference. Units are numbered along a position from the lowest X (stage left) upwards. A fixture without a Position Reference is listed under **No position**. Venue objects are scenery and are not listed.

An unpatched fixture still hangs, so every report lists it. Its DMX address reads **Unpatched**, highlighted, and the DMX Sheet lists it after all patched splits.

## Export

- **Export CSV** saves the shown report as a CSV file named after it.
- **Copy CSV** copies the same CSV to the clipboard, for pasting into a spreadsheet.
- **Print / PDF** prints only the report, with its title and date. Choose **Save as PDF** in the print dialog for a PDF.

Plans with fixture symbols and labels print from Architect's print pages; see [Draw and print plans](../05-draw-and-print-plans.md).
