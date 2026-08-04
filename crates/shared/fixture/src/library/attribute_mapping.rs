use super::FixtureLibrary;
use crate::FixtureError;
use rusqlite::params;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureSourceMappingPreference {
    pub source_format: String,
    pub source_attribute: String,
    pub target_attribute: String,
}

impl FixtureLibrary {
    pub fn source_mapping_preferences(
        &self,
    ) -> Result<Vec<FixtureSourceMappingPreference>, FixtureError> {
        let mut statement = self.conn.prepare(
            "SELECT source_format,source_attribute,target_attribute FROM fixture_attribute_mapping_preferences ORDER BY source_format,source_attribute",
        )?;
        Ok(statement
            .query_map([], |row| {
                Ok(FixtureSourceMappingPreference {
                    source_format: row.get(0)?,
                    source_attribute: row.get(1)?,
                    target_attribute: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn set_source_mapping_preference(
        &self,
        source_format: &str,
        source_attribute: &str,
        target_attribute: Option<&str>,
    ) -> Result<Option<FixtureSourceMappingPreference>, FixtureError> {
        let Some(target_attribute) = target_attribute else {
            self.conn.execute(
                "DELETE FROM fixture_attribute_mapping_preferences WHERE source_format=?1 AND source_attribute=?2",
                params![source_format, source_attribute],
            )?;
            return Ok(None);
        };
        self.conn.execute(
            "INSERT INTO fixture_attribute_mapping_preferences(source_format,source_attribute,target_attribute) VALUES(?1,?2,?3) ON CONFLICT(source_format,source_attribute) DO UPDATE SET target_attribute=excluded.target_attribute",
            params![source_format, source_attribute, target_attribute],
        )?;
        Ok(Some(FixtureSourceMappingPreference {
            source_format: source_format.into(),
            source_attribute: source_attribute.into(),
            target_attribute: target_attribute.into(),
        }))
    }
}
