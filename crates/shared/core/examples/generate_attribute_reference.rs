use light_core::{ATTRIBUTE_REGISTRY, AttributeConfiguration, EncoderGroup};
use std::{collections::HashSet, env, fs, path::Path};

fn uses_semantic_pages(group: EncoderGroup) -> bool {
    matches!(
        group,
        EncoderGroup::Color | EncoderGroup::Position | EncoderGroup::Focus
    )
}

fn encoder_group_label(group: EncoderGroup) -> &'static str {
    match group {
        EncoderGroup::Intensity => "Intensity",
        EncoderGroup::Color => "Color",
        EncoderGroup::Position => "Position",
        EncoderGroup::Beam => "Beam",
        EncoderGroup::Shapers => "Shapers",
        EncoderGroup::Focus => "Focus",
        EncoderGroup::Control => "Control",
        EncoderGroup::Media => "Media",
    }
}

fn label_for(id: &str) -> String {
    ATTRIBUTE_REGISTRY
        .iter()
        .find(|descriptor| descriptor.id == id)
        .map(|descriptor| descriptor.label.to_owned())
        .unwrap_or_else(|| id.to_owned())
}

fn attribute_reference(id: &str) -> String {
    format!("**{}** (`{id}`)", label_for(id))
}

fn main() {
    let output = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: generate_attribute_reference <output-path>");
        std::process::exit(2);
    });
    let configuration = AttributeConfiguration::recommended();
    let mut placements = configuration.placements;
    placements.sort_by_key(|placement| {
        (
            placement.encoder.group,
            placement.encoder.page,
            placement.encoder.slot,
        )
    });
    let semantic_attributes: HashSet<_> = placements
        .iter()
        .filter(|placement| uses_semantic_pages(placement.encoder.group))
        .map(|placement| placement.attribute.0.clone())
        .collect();

    let mut document = String::from(
        "# Default attributes and encoders\n\n<!-- Generated from AttributeConfiguration::recommended() by the generate_attribute_reference example. Do not edit by hand. -->\n\nColor, Position, and Focus use selection-dependent controls. See [Color Intent](../20-ToskLight-Control/20-Programmer-and-Cues/05-color-intent.md), [Position and aiming](../20-ToskLight-Control/20-Programmer-and-Cues/08-position.md), and [Focus and Zoom](../20-ToskLight-Control/20-Programmer-and-Cues/06-focus-and-zoom.md) for their current pages, units, and activation behavior.\n\nThe tables list the remaining registry defaults. **Restore recommended defaults** restores those assignments; the current desk may use a different layout or activation grouping.\n\n## Other encoder assignments\n\n| Encoder group | Encoder | Attribute | Push-turn companion |\n| --- | --- | --- | --- |\n",
    );
    for placement in placements {
        if uses_semantic_pages(placement.encoder.group) {
            continue;
        }
        let push_turn = placement
            .push_turn_of
            .as_ref()
            .map(|attribute| attribute_reference(&attribute.0))
            .unwrap_or_else(|| "—".to_owned());
        document.push_str(&format!(
            "| {} | P{}/E{} | {} | {} |\n",
            encoder_group_label(placement.encoder.group),
            placement.encoder.page,
            placement.encoder.slot,
            attribute_reference(&placement.attribute.0),
            push_turn,
        ));
    }

    document.push_str(
        "\n## Default attribute activation groups\n\n| Group | Attributes activated together |\n| --- | --- |\n",
    );
    for group in configuration.activation_groups {
        if group
            .members
            .iter()
            .any(|attribute| semantic_attributes.contains(&attribute.0))
        {
            continue;
        }
        let members = group
            .members
            .iter()
            .map(|attribute| attribute_reference(&attribute.0))
            .collect::<Vec<_>>()
            .join("<br>");
        document.push_str(&format!("| **{}** | {} |\n", group.label, members));
    }

    let output = Path::new(&output);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).expect("create generated reference directory");
    }
    fs::write(output, document).expect("write generated attribute reference");
}
