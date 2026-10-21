# Configure the Desk

Desk Setup contains installation settings that do not travel with a show. Open **Show > Enter Setup** before patching to configure screens, inputs, output, and recovery.

![Show menu with Enter Setup](../../assets/screenshots/workflows/show-menu.png)

Work through:

1. [Screens and Desktop Layouts](02-screens-and-layouts.md)
2. [Inputs, Extensions, and Network Control](03-inputs-extensions-and-network.md)
3. [DMX Output and Universe Routes](04-dmx-output.md)
4. [Surfaces, Sessions, and Recovery](05-users-sessions-and-recovery.md)
5. [Media Server connection](../../30-ToskLight-Media/09-connect-the-desk.md)

## Preferences

**Preferences** has **Defaults**, **Attributes & encoders**, **Highlight**, and **Others** pages. Defaults includes **Record & Update**, **Playback**, and **Pools**. Pool defaults set tile widths and colors; individual panes can override their layout.

Changes normally save immediately. If a setting reports **Restart required**, finish setup, restart once, and check output again.

## Attributes and encoders

Select **Encoder groups**, **Attribute activation groups**, or **Attributes** from the window title.

- **Encoder groups:** choose **Assign attribute** on an empty slot, or drag an assigned encoder to another slot, page, or group. Releasing outside a slot cancels the move. Use **Screens & playback > Configure encoder placement** to change the number of visible encoders.
- **Attribute activation groups:** start from **None**, **All**, **By Encoder Group**, or **Intelligent**, then edit membership. A preset replaces the existing groups. Each attribute can belong to one activation group.
- **Attributes:** inspect built-in and custom controls. **New custom attribute** adds a show-owned control; **Imported attribute names** maps an import's different spelling to an existing control.

Imported-name mappings belong to the desk. Prefer mapping an existing capability over creating a duplicate. See [Attributes and Activation](14-attribute-reference-and-activation.md).
