# Operate Outputs, Master, and Layers

The operator surface separates the selected output's Master from its ordered layers. DMX remains the normal show-control owner; browser takeover is a deliberate temporary override for setup and diagnosis.

> [!danger] Missing graphic
> Add an annotated Pixel playback screenshot showing Master, ordered layers, takeover ownership, isolated layer preview, composite preview, and physical output.

## Take control

Take over the output or exact layer before changing it from the browser. The surface identifies browser ownership so a technician does not mistake a local preview action for a Desk command. Release takeover when the Desk should become authoritative again.

## Choose content

A layer selects one addressed content folder/file and an optional mask folder/file. Content `0` or `255` is blank. The isolated layer preview proves that layer's source and transformations; the composite preview proves the result after layer order, blending, effects, and the Master are applied.

## Transform and shape a layer

Layer controls include position, scale, rotation, opacity, crop or shaper behavior, blending, color treatment, and typed effects supported by the active personality. Edit one control at a time while watching both the isolated and composite previews. A correct isolated layer can still disappear in the composite because of its order, blend, opacity, mask, Master, or another layer.

## Master output

The Master controls the output-wide result and owns output-level configuration exposed by the selected personality. A layer can be correct and still remain black when the Master is black, stopped, or owned by another source.

To return to show control, release the browser takeover rather than trying to reproduce the Desk's last DMX values manually. The next valid Art-Net or sACN frame then remains the authoritative input.
