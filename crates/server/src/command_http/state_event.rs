use light_programmer::CommandLineState;
use light_wire::v2::command_line::{
    CommandHttpSource, CommandLineChangedEvent, CommandTarget as WireCommandTarget,
};

use super::super::{AppState, Session};

pub(super) fn publish_command_line_change(
    state: &AppState,
    session: &Session,
    before: &CommandLineState,
    after: &CommandLineState,
    source: &str,
    request_id: Option<&str>,
) {
    if before == after {
        return;
    }
    super::super::emit_update_armed_transition(
        state,
        session,
        super::super::command_line_arms_update(before.visible_text()),
        super::super::command_line_arms_update(after.visible_text()),
        source,
    );
    let (retained_text, sensitive) = super::super::command_audit_projection(after.visible_text());
    let event = CommandLineChangedEvent {
        desk_id: session.desk.id,
        session_id: session.id.0,
        user_id: session.user.id.0,
        text: retained_text_if_sensitive(after, retained_text, sensitive),
        target: wire_target(after.target),
        pristine: after.pristine,
        revision: after.revision,
        source: wire_source(source),
        request_id: request_id.map(str::to_owned),
        redacted: sensitive,
    };
    super::super::emit(
        state,
        "command_line_changed",
        serde_json::to_value(event).expect("command-line wire events serialize"),
    );
}

fn retained_text_if_sensitive(
    state: &CommandLineState,
    retained_text: String,
    sensitive: bool,
) -> String {
    if sensitive {
        retained_text
    } else {
        state.visible_text().to_owned()
    }
}

const fn wire_target(target: light_programmer::CommandTarget) -> WireCommandTarget {
    match target {
        light_programmer::CommandTarget::Fixture => WireCommandTarget::Fixture,
        light_programmer::CommandTarget::Group => WireCommandTarget::Group,
    }
}

fn wire_source(source: &str) -> CommandHttpSource {
    match source {
        "http" => CommandHttpSource::Http,
        "http_key" => CommandHttpSource::HttpKey,
        _ => unreachable!("the command HTTP adapter has a bounded source enum"),
    }
}
