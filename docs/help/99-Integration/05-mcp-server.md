# MCP server

ToskLight ships an MCP server, `tosklight`, that lets an AI assistant program the desk and edit the patch. It uses the same desk routes as the keypad, OSC surfaces and the Control app, so it can do only what an operator can do, under the same rules.

## Connect it

Run `apps/patch-mcp/src/main.ts` (package `@tosklight/patch-mcp`, command `tosklight-patch-mcp`) from the MCP client over stdio. Set:

| Setting | Meaning |
| --- | --- |
| `TOSKLIGHT_URL` | Where the desk listens. Default `http://127.0.0.1:5000`. |
| `TOSKLIGHT_DESK_ID` | The desk alias the server uses. Set it to your desk's alias to share that desk's command line, selection and Programmer. Any other alias is a separate desk with its own command line. |
| `TOSKLIGHT_TARGET` | `architect` edits the patch of the Architect open on this computer instead. The programming tools need the Control desk. |

One desk alias is one desk. With your desk's alias, every line the assistant types runs on the same command line your desk shows, and appears in that desk's **Command Line History**.

## Safety

- A command that deletes, moves, copies, updates, or records over existing programming is refused unless the call passes `confirm: true`. Recording a new Cue, Group or Preset, selecting, and setting values never need it.
- `update_cue` always needs `confirm: true`. `preload` with `go` needs it too, because Preload GO changes live output.
- Use `preload` with `enter` to program blind. Values then go into Preload, and live output does not change until Preload GO.
- Every change is ordinary desk programming, so Cuelist and object undo work as usual.

## Example

"Warm, tighter beams on the drummer, 3 s fade, record as Cue 5" becomes:

1. `select` with the drummer's Group.
2. `set_values` with `color`, `zoom_degrees` and `fade_seconds: 3`.
3. `snapshot` to check the look.
4. `record_cue` with `cue_list`, `cue: "5"` and `fade_seconds: 3`.

## Programming tools

The value tools act on the current selection. Colour values are Color Intent in percent, position values are Pan and Tilt in degrees, and zoom is the beam angle in degrees, as on the keypad (see [the command line](../20-ToskLight-Control/20-Programmer-and-Cues/01-command-line.md#setting-non-intensity-values)). `snapshot` returns a PNG of the rig from above: each fixture at its stage position, filled with its output colour and as bright as its intensity. A running fade shows its current point.

#### `get_show_summary`

The open show, how much it holds, the shared command line, and whether Preload (blind programming) is pending.

No arguments.

#### `list_groups`

Every Group with its number, name and ordered fixture numbers. Order matters: spreads and THRU values follow it.

No arguments.

#### `list_presets`

Stored Presets with family, number and name. Recall one with apply_preset.

| Argument | Type | Meaning |
| --- | --- | --- |
| `family` | `intensity`, `color`, `position`, `beam`, `focus`, `mixed` |  |

#### `list_cue_lists`

Cuelists with their Cues, timing and how many values each Cue stores. Pass cue_list and include_values for the stored values of one Cuelist.

| Argument | Type | Meaning |
| --- | --- | --- |
| `cue_list` | number | Cuelist pool number; omit for all Cuelists. |
| `include_values` | boolean | Include every stored fixture and Group value per Cue. |

#### `list_playbacks`

Playback pages and what each playback position runs.

No arguments.

#### `get_programmer`

The current selection (in order) and the Programmer's fixture and Group values, live and pending in Preload.

No arguments.

#### `get_command_line`

The desk's shared command line as every surface shows it.

No arguments.

#### `run_command`

Type a line on the desk's one shared command line and press ENT, with the keypad's own grammar (e.g. `GROUP 3 AT 50`, `FIXTURE 1 THRU 4 AT POSITION 30 DIV 10`, `RECORD CUELIST 4 CUE 5 TIME 3`). Lines that delete, move, copy, update or record over existing programming need confirm: true.

| Argument | Type | Meaning |
| --- | --- | --- |
| `command` | string (required) |  |
| `confirm` | boolean | Required as true when the command would overwrite, update, move or delete stored programming. |

#### `select`

Select fixtures or a Group on the shared command line. `fixtures` uses keypad syntax such as `1 THRU 6 + 9`.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixtures` | string | Fixture numbers and ranges, e.g. `1 THRU 6 + 9`. |
| `group` | number or string | A Group number. |

#### `set_values`

Set values on the current selection. Intensity in percent; color as Color Intent red/green/blue/white in percent; position as Pan/Tilt in degrees; zoom in beam degrees; focus in percent. An optional fade applies to each value.

| Argument | Type | Meaning |
| --- | --- | --- |
| `intensity` | number |  |
| `color` | object | Fields: `red`, `green`, `blue`, `white` |
| `position` | object | Fields: `pan`, `tilt` |
| `zoom_degrees` | number |  |
| `focus` | number |  |
| `fade_seconds` | number |  |

#### `apply_preset`

Recall a Preset onto the current selection, e.g. family color, number 3.

| Argument | Type | Meaning |
| --- | --- | --- |
| `family` | `intensity`, `color`, `position`, `beam`, `focus` (required) |  |
| `number` | number (required) |  |
| `fade_seconds` | number |  |

#### `clear`

Clear the selection, or with values: true also the Programmer's values (CLR pressed twice).

| Argument | Type | Meaning |
| --- | --- | --- |
| `values` | boolean |  |

#### `record_cue`

Record the Programmer as a Cue. Without `cue` it appends to the Cuelist. Recording onto an existing Cue number needs confirm: true. `merge` adds to an existing Cue instead of replacing it (still needs confirm).

| Argument | Type | Meaning |
| --- | --- | --- |
| `cue_list` | number (required) |  |
| `cue` | string | Cue number such as `5` or `2.1`. |
| `fade_seconds` | number |  |
| `merge` | boolean |  |
| `confirm` | boolean | Required as true when the command would overwrite, update, move or delete stored programming. |

#### `update_cue`

Update an existing Cue from the Programmer. mode: update (values the Cue already stores), tracked (the Cue each value comes from), known (attributes the Cuelist already knows), all (everything). Always needs confirm: true.

| Argument | Type | Meaning |
| --- | --- | --- |
| `cue_list` | number (required) |  |
| `cue` | string (required) |  |
| `mode` | `update`, `tracked`, `known`, `all` |  |
| `confirm` | boolean | Required as true when the command would overwrite, update, move or delete stored programming. |

#### `playback`

Operate a playback: go, back, release, or set its master fader (0–100%). Without page, the desk's current page is used.

| Argument | Type | Meaning |
| --- | --- | --- |
| `position` | number (required) |  |
| `page` | number |  |
| `action` | `go`, `back`, `release`, `master` (required) |  |
| `value` | number | Master level in percent for action master. |

#### `preload`

Blind programming. enter: the following values go into Preload and do not change live output. go: apply everything pending at once (changes live output; needs confirm: true). clear: drop everything pending.

| Argument | Type | Meaning |
| --- | --- | --- |
| `action` | `enter`, `go`, `clear` (required) |  |
| `confirm` | boolean | Required as true when the command would overwrite, update, move or delete stored programming. |

#### `snapshot`

A top-view picture of the rig as it currently looks: each fixture at its stage position, coloured and brightened by its output (or its pending Preload with preload: true). A running fade shows its current point. Use it to check a look before recording it.

| Argument | Type | Meaning |
| --- | --- | --- |
| `preload` | boolean |  |

## Patch, Venue and media tools

These tools edit the patch, Venue objects and the Architect's media layout. Fixtures are named by the number an operator says out loud, or `0.N` for a Venue object.

#### `search_fixture_library`

Search the shipped and imported fixture library. Returns the profile and mode ids that adding a fixture needs.

| Argument | Type | Meaning |
| --- | --- | --- |
| `query` | string | Matched against manufacturer, name and mode. Omit to list everything. |
| `limit` | number |  |

#### `list_fixtures`

Every fixture and Venue object in the show, with the fields these tools can change. A Venue object reports its kind, size in metres, colour, chain ends and model scale.

No arguments.

#### `add_fixture`

Add a fixture or a Venue object (curtain, truss, riser, chain, railing…) to the show. Name the profile by profile_name — its newest revision and first mode unless told otherwise — or by the ids search_fixture_library returns. A visual-only Venue object gets the next free 0.N number and no DMX address; a patchable fixture the next free fixture number when none is given.

| Argument | Type | Meaning |
| --- | --- | --- |
| `profile_name` | string | The profile's name, e.g. "Curtain". Resolves to its newest revision. Use this or profile_id. |
| `manufacturer` | string | Narrows profile_name when two makers share it. |
| `profile_id` | string |  |
| `profile_revision` | number | Defaults to the newest revision. |
| `mode_id` | string |  |
| `mode_name` | string | Defaults to the profile's first mode. |
| `fixture_number` | number or string | Omit for the next free number. A visual-only Venue object takes a 0.N number; a patchable fixture a positive one. |
| `name` | string | Defaults to the profile's name. |
| `note` | string |  |
| `layer_id` | string | Defaults to `default`. |
| `universe` | number | Not for a visual-only Venue object. |
| `address` | number |  |
| `x` | number | Millimetres across the stage. |
| `y` | number | Millimetres upstage. |
| `z` | number | Millimetres up. |
| `rotation_x` | number |  |
| `rotation_y` | number |  |
| `rotation_z` | number |  |
| `gel` | object | Open white when omitted. A built-in gel names a catalog entry; a custom gel carries its own sRGB colour. |
| `size_metres` | object | A Venue object's size in metres. Only the measurements its profile makes adjustable can be set, within the profile's minimum and maximum. |
| `colour` | string or null | A Venue object's colour as #RRGGBB. Null keeps its kind's own material. |
| `chain_mode` | `plain`, `motor_top`, `motor_bottom` | How a chain is rigged; sets both ends the way the patch sheet does. |
| `chain_top` | `motor`, `direct`, `steelflex_loop` |  |
| `chain_bottom` | `direct`, `steelflex_loop`, `motor` |  |
| `model_scale` | number or null | How many times its built size the object is drawn, 0.01 to 100. Null is 1. |

#### `remove_fixture`

Remove a fixture from the show.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |

#### `set_fixture_placement`

Set a fixture's position, rotation and bracket angle. Omitted values are left alone.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `x` | number |  |
| `y` | number |  |
| `z` | number |  |
| `rotation_x` | number |  |
| `rotation_y` | number |  |
| `rotation_z` | number |  |
| `bracket_angle` | number | Degrees, positive nose-down. |

#### `set_fixture_shaper_and_gel`

Set a fixture's shaper angle and its gel.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `shaper_angle` | number | Degrees. Null clears a fitted shaper. |
| `gel` | object | Open white when omitted. A built-in gel names a catalog entry; a custom gel carries its own sRGB colour. |

#### `set_fixture_identity`

Set a fixture's name and its note.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `name` | string |  |
| `note` | string | The operator's own note for this fixture, not the profile's shared notes. |

#### `set_fixture_patch`

Set a fixture's universe and address, and whether its pan or tilt is reversed.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `universe` | number | Null unpatches the fixture without removing it. |
| `address` | number |  |
| `invert_pan` | boolean |  |
| `invert_tilt` | boolean |  |

#### `add_multipatch`

Add another physical instance controlled and selected as this fixture. An instance with no address exists in the visualizer only.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `name` | string (required) |  |
| `universe` | number |  |
| `address` | number |  |
| `x` | number |  |
| `y` | number |  |
| `z` | number |  |

#### `assign_position_master`

Slave a fixture to a 3D Point, so moving or rotating that point carries the fixture with it. Pass no master to unslave it.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `master_fixture_number` | number or string | The 3D Point's fixture number. Omit to unslave. |

#### `list_layers`

The patch layers the show stores, in their own order, each with the number of fixtures standing on it. A named layer with no fixtures is listed too.

No arguments.

#### `save_layer`

Create a layer, or rename and reorder one that exists. The id is the layer's stable name in the patch; `name` is what an operator reads and `order` is where it sits in the list.

| Argument | Type | Meaning |
| --- | --- | --- |
| `layer_id` | string (required) | The layer's stable id. A new id creates a layer. |
| `name` | string (required) |  |
| `order` | number | Where the layer sits. Lower comes first. |

#### `remove_layer`

Empty a layer by moving every fixture on it to another layer. The desk keeps the layer record itself; there is no route that deletes one.

| Argument | Type | Meaning |
| --- | --- | --- |
| `layer_id` | string (required) |  |
| `move_to_layer_id` | string (required) | Where the fixtures go. Must be a different layer. |

#### `set_fixture_layer`

Move one fixture onto a layer. The layer should exist already; use save_layer to create it.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `layer_id` | string (required) |  |

#### `set_venue_size`

Set a Venue object's size in metres — a curtain's width and height, a truss's length, a chain's drop. Only the measurements its profile makes adjustable can be set; a size outside the profile's range is refused with that range. Omitted measurements are left alone.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `width` | number | Width, in metres. |
| `height` | number | Height, in metres. |
| `depth` | number | Depth, in metres. |

#### `set_venue_options`

Set a Venue object's colour, and a chain's end fittings. Colour null gives the object its kind's own material back. Omitted options are left alone.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `colour` | string or null | A Venue object's colour as #RRGGBB. Null keeps its kind's own material. |
| `chain_mode` | `plain`, `motor_top`, `motor_bottom` | How a chain is rigged; sets both ends the way the patch sheet does. |
| `chain_top` | `motor`, `direct`, `steelflex_loop` |  |
| `chain_bottom` | `direct`, `steelflex_loop`, `motor` |  |

#### `set_model_scale`

Draw a placed object at a multiple of the size it was built at, from 0.01 to 100. Null draws it at its built size.

| Argument | Type | Meaning |
| --- | --- | --- |
| `fixture_number` | number or string (required) | The fixture number an operator would say out loud, or a Venue object's 0.N number (pass "0.10" as a string). |
| `model_scale` | number or null (required) | How many times its built size the object is drawn, 0.01 to 100. Null is 1. |

#### `get_media_layout`

The Architect's media layout: media servers, their sources, LED module types, surfaces with their sections, and projectors, each with its id and revision.

No arguments.

#### `put_media_server`

Create or update a media server the show sends video to, reached over CITP.

| Argument | Type | Meaning |
| --- | --- | --- |
| `id` | string | The object's id. Omit to create a new one; give an existing id to update it. |
| `name` | string |  |
| `citp_host` | string | Defaults to 127.0.0.1. |
| `citp_port` | number | Defaults to 4809. |
| `discovery_identity` | string |  |
| `last_known_endpoint` | string |  |

#### `put_media_source`

Create or update a source a media server advertises. The numeric advertised source id is what identifies it on the server.

| Argument | Type | Meaning |
| --- | --- | --- |
| `id` | string | The object's id. Omit to create a new one; give an existing id to update it. |
| `server_id` | string |  |
| `advertised_source_id` | number |  |
| `name` | string |  |
| `output_name` | string |  |
| `width` | number | Pixels. |
| `height` | number | Pixels. |
| `aspect_ratio` | number |  |

#### `put_led_module_type`

Create or update an LED module type that LED sections are built from.

| Argument | Type | Meaning |
| --- | --- | --- |
| `id` | string | The object's id. Omit to create a new one; give an existing id to update it. |
| `name` | string |  |
| `width_metres` | number |  |
| `height_metres` | number |  |
| `pixel_pitch_millimetres` | number |  |
| `horizontal_gap_metres` | number |  |
| `vertical_gap_metres` | number |  |
| `pixel_width` | number |  |
| `pixel_height` | number |  |

#### `put_media_surface`

Create or update a surface a source is shown on, made of projection-screen, TV and LED sections. Sections, when given, replace the surface's sections.

| Argument | Type | Meaning |
| --- | --- | --- |
| `id` | string | The object's id. Omit to create a new one; give an existing id to update it. |
| `name` | string |  |
| `source_id` | string or null | The source shown. Null shows none. |
| `sections` | array |  |

#### `put_media_projector`

Create or update a projector that lights a surface.

| Argument | Type | Meaning |
| --- | --- | --- |
| `id` | string | The object's id. Omit to create a new one; give an existing id to update it. |
| `name` | string |  |
| `surface_id` | string |  |
| `position_metres` | array |  |
| `rotation_degrees` | array |  |
| `body_model` | string | Defaults to `projector`. |
| `throw_ratio` | number | Defaults to 1.5. |
| `lens_shift` | array |  |
| `cone_length_metres` | number | Defaults to 12. |
| `spill` | number |  |

#### `delete_media_object`

Delete one media layout object. Deleting a server removes its sources too; a surface that showed one keeps its fallback image.

| Argument | Type | Meaning |
| --- | --- | --- |
| `kind` | `media_server`, `media_source`, `led_module_type`, `media_surface`, `media_projector` (required) |  |
| `id` | string (required) |  |
