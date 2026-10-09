//! Portable Cuelist number resolution. These addresses never select physical Playback targets.
use crate::{ActionError, ActionErrorKind, lossless_json};
use light_playback::{CueList, CueListPoolCatalog, PlaybackDefinition};
use light_show::{PortableShowDocument, PortableShowTransaction};

pub fn cuelist_pool_catalog(
    document: &PortableShowDocument,
) -> Result<CueListPoolCatalog, ActionError> {
    let lists = document
        .objects_of_kind("cue_list")
        .map(|object| serde_json::from_value::<CueList>(object.body().clone()).map_err(invalid))
        .collect::<Result<Vec<_>, _>>()?;
    let playbacks = document
        .objects_of_kind("playback")
        .map(|object| {
            serde_json::from_value::<PlaybackDefinition>(object.body().clone()).map_err(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    CueListPoolCatalog::build(
        &lists,
        &playbacks,
        document.objects_of_kind("playback_page").next().is_none(),
    )
    .map_err(invalid)
}

/// Author address metadata only on an independent-numbering mutation. Every existing numeric
/// alias survives, unknown body fields survive, and physical assignments/pages stay untouched.
pub(crate) fn migrate_cuelist_pool(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
    catalog: &CueListPoolCatalog,
    except: light_core::CueListId,
) -> Result<Vec<String>, ActionError> {
    let mut changed = Vec::new();
    for object in document.objects_of_kind("cue_list") {
        let original: CueList = serde_json::from_value(object.body().clone()).map_err(invalid)?;
        if original.id == except {
            continue;
        }
        let mut migrated = original.clone();
        catalog.migrate(&mut migrated).map_err(invalid)?;
        if original != migrated {
            let body =
                lossless_json::merge_typed(object.body(), &original, &migrated).map_err(invalid)?;
            transaction.put("cue_list", object.key().id().to_owned(), body);
            changed.push(object.key().id().to_owned());
        }
    }
    Ok(changed)
}
fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}
