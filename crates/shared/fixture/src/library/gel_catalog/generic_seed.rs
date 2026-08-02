use super::FixtureLibrary;
use crate::FixtureError;
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

const GENERIC_GEL_CATALOG_SEED_KEY: &str = "generic_gel_catalog_seed_version";
const GENERIC_GEL_CATALOG_SEED_VERSION: &str = "1";
const GENERIC_GEL_CATALOG_NAME: &str = "Generic gels";

pub const GENERIC_GEL_CATALOG_ID: Uuid = Uuid::from_u128(0xa965_9357_95f8_4b47_a698_8d5d_2401_cbe5);

pub const GENERIC_GEL_ENTRY_IDS: [Uuid; 16] = [
    Uuid::from_u128(0xe484_6037_e200_4dd9_bfb6_3f8c_94ff_f186),
    Uuid::from_u128(0xd9cb_6d0a_8663_4f82_ba6e_edb7_b11c_d5d3),
    Uuid::from_u128(0x5c23_8d93_8c41_46b9_b457_dff7_b276_102e),
    Uuid::from_u128(0x7827_e921_2394_4204_830e_d46c_a9e0_da5e),
    Uuid::from_u128(0x33a3_7787_ba78_4890_8d44_f211_bf8b_7990),
    Uuid::from_u128(0xa4d5_a1bf_d823_4960_b9fb_579f_e3c6_c53b),
    Uuid::from_u128(0x9f14_d9dc_6ac8_467e_8504_8624_53b6_17fc),
    Uuid::from_u128(0x765c_d408_5f0f_46ba_ace7_e2cd_a26b_537b),
    Uuid::from_u128(0x1ed8_58c4_7f0a_4f0f_9605_8232_bee2_0a50),
    Uuid::from_u128(0x543e_3532_aded_4f52_aeeb_0851_5d25_8403),
    Uuid::from_u128(0x6b23_3bb4_a660_4853_ae2e_d9ce_309f_4239),
    Uuid::from_u128(0x11f7_5e8b_0684_4b4a_afe5_484c_3764_bb8d),
    Uuid::from_u128(0x4815_f198_728e_4081_b81b_00aa_16ee_c2d2),
    Uuid::from_u128(0x18a6_8991_de2c_40c6_bc84_54bb_e1ab_fc61),
    Uuid::from_u128(0x01d4_fb38_d2c9_4ffc_8e65_4f73_77a3_f419),
    Uuid::from_u128(0x09a5_7432_50c3_4c9a_8d02_0932_bc68_7d46),
];

struct SeedGelEntry {
    number: &'static str,
    name: &'static str,
    display_srgb: &'static str,
    visualizer_srgb: &'static str,
}

const GENERIC_GEL_ENTRIES: [SeedGelEntry; 16] = [
    SeedGelEntry {
        number: "G00",
        name: "Open white",
        display_srgb: "#FFFFFF",
        visualizer_srgb: "#FFFFFF",
    },
    SeedGelEntry {
        number: "G01",
        name: "Red",
        display_srgb: "#E5484D",
        visualizer_srgb: "#FF2020",
    },
    SeedGelEntry {
        number: "G02",
        name: "Orange",
        display_srgb: "#E87932",
        visualizer_srgb: "#FF5A10",
    },
    SeedGelEntry {
        number: "G03",
        name: "Amber",
        display_srgb: "#D89A28",
        visualizer_srgb: "#FF8A18",
    },
    SeedGelEntry {
        number: "G04",
        name: "Yellow",
        display_srgb: "#D8C832",
        visualizer_srgb: "#FFE52A",
    },
    SeedGelEntry {
        number: "G05",
        name: "Green",
        display_srgb: "#3DAA5A",
        visualizer_srgb: "#20D840",
    },
    SeedGelEntry {
        number: "G06",
        name: "Cyan",
        display_srgb: "#2FB6B4",
        visualizer_srgb: "#20D8D8",
    },
    SeedGelEntry {
        number: "G07",
        name: "Blue",
        display_srgb: "#3E6FD8",
        visualizer_srgb: "#2850FF",
    },
    SeedGelEntry {
        number: "G08",
        name: "Violet",
        display_srgb: "#7B52C7",
        visualizer_srgb: "#7A35E8",
    },
    SeedGelEntry {
        number: "G09",
        name: "Magenta",
        display_srgb: "#C64FA8",
        visualizer_srgb: "#E832C8",
    },
    SeedGelEntry {
        number: "G10",
        name: "Pink",
        display_srgb: "#D86F9E",
        visualizer_srgb: "#FF6AAA",
    },
    SeedGelEntry {
        number: "G11",
        name: "Warm white",
        display_srgb: "#F2C9A0",
        visualizer_srgb: "#FFD0A0",
    },
    SeedGelEntry {
        number: "G12",
        name: "Cool white",
        display_srgb: "#D5E8F5",
        visualizer_srgb: "#D8EEFF",
    },
    SeedGelEntry {
        number: "G13",
        name: "Pale amber",
        display_srgb: "#D9B877",
        visualizer_srgb: "#FFC878",
    },
    SeedGelEntry {
        number: "G14",
        name: "Pale blue",
        display_srgb: "#8EB7D8",
        visualizer_srgb: "#90C8FF",
    },
    SeedGelEntry {
        number: "G15",
        name: "Deep blue",
        display_srgb: "#2B3F8F",
        visualizer_srgb: "#1628C8",
    },
];

impl FixtureLibrary {
    pub(in crate::library) fn seed_generic_gel_catalog(&self) -> Result<(), FixtureError> {
        let transaction = self.conn.unchecked_transaction()?;
        let already_seeded = transaction
            .query_row(
                "SELECT value FROM library_metadata WHERE key=?1",
                [GENERIC_GEL_CATALOG_SEED_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .is_some();
        if already_seeded {
            transaction.commit()?;
            return Ok(());
        }

        let catalog_id = GENERIC_GEL_CATALOG_ID.to_string();
        let catalog_exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM gel_catalogs WHERE id=?1)",
            [&catalog_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !catalog_exists {
            transaction.execute(
                "INSERT INTO gel_catalogs(id,name,revision) VALUES(?1,?2,1)",
                params![catalog_id, GENERIC_GEL_CATALOG_NAME],
            )?;
            for (sort_order, (entry_id, entry)) in GENERIC_GEL_ENTRY_IDS
                .iter()
                .zip(GENERIC_GEL_ENTRIES.iter())
                .enumerate()
            {
                transaction.execute(
                    "INSERT INTO gel_catalog_entries(catalog_id,entry_id,number,name,display_srgb,visualizer_srgb,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        catalog_id,
                        entry_id.to_string(),
                        entry.number,
                        entry.name,
                        entry.display_srgb,
                        entry.visualizer_srgb,
                        sort_order as i64,
                    ],
                )?;
            }
        }
        transaction.execute(
            "INSERT INTO library_metadata(key,value) VALUES(?1,?2)",
            params![
                GENERIC_GEL_CATALOG_SEED_KEY,
                GENERIC_GEL_CATALOG_SEED_VERSION
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{GelCatalog, GelCatalogEntry, GelCatalogImportTarget};
    use super::*;
    use rusqlite::Connection;
    use std::path::PathBuf;

    const HEADER: &str = "number,name,display_rgb,visualizer_rgb\n";

    fn temporary_library_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("fixture-library-{label}-{}.sqlite", Uuid::new_v4()))
    }

    #[test]
    fn first_open_seeds_the_exact_reviewed_generic_inventory_with_stable_ids() {
        let path = temporary_library_path("generic-gels");
        let library = FixtureLibrary::open(&path).unwrap();
        let catalog = library
            .gel_catalog(GENERIC_GEL_CATALOG_ID)
            .unwrap()
            .unwrap();

        assert_eq!(catalog.revision, 1);
        assert_eq!(catalog.name, "Generic gels");
        assert_eq!(
            catalog
                .entries
                .iter()
                .map(|entry| (
                    entry.id,
                    entry.number.as_str(),
                    entry.name.as_str(),
                    entry.display_srgb.as_str(),
                    entry.visualizer_srgb.as_str(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    GENERIC_GEL_ENTRY_IDS[0],
                    "G00",
                    "Open white",
                    "#FFFFFF",
                    "#FFFFFF"
                ),
                (GENERIC_GEL_ENTRY_IDS[1], "G01", "Red", "#E5484D", "#FF2020"),
                (
                    GENERIC_GEL_ENTRY_IDS[2],
                    "G02",
                    "Orange",
                    "#E87932",
                    "#FF5A10"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[3],
                    "G03",
                    "Amber",
                    "#D89A28",
                    "#FF8A18"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[4],
                    "G04",
                    "Yellow",
                    "#D8C832",
                    "#FFE52A"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[5],
                    "G05",
                    "Green",
                    "#3DAA5A",
                    "#20D840"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[6],
                    "G06",
                    "Cyan",
                    "#2FB6B4",
                    "#20D8D8"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[7],
                    "G07",
                    "Blue",
                    "#3E6FD8",
                    "#2850FF"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[8],
                    "G08",
                    "Violet",
                    "#7B52C7",
                    "#7A35E8"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[9],
                    "G09",
                    "Magenta",
                    "#C64FA8",
                    "#E832C8"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[10],
                    "G10",
                    "Pink",
                    "#D86F9E",
                    "#FF6AAA"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[11],
                    "G11",
                    "Warm white",
                    "#F2C9A0",
                    "#FFD0A0"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[12],
                    "G12",
                    "Cool white",
                    "#D5E8F5",
                    "#D8EEFF"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[13],
                    "G13",
                    "Pale amber",
                    "#D9B877",
                    "#FFC878"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[14],
                    "G14",
                    "Pale blue",
                    "#8EB7D8",
                    "#90C8FF"
                ),
                (
                    GENERIC_GEL_ENTRY_IDS[15],
                    "G15",
                    "Deep blue",
                    "#2B3F8F",
                    "#1628C8"
                ),
            ]
        );

        drop(library);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reopening_keeps_the_seeded_catalog_revision_inventory_and_marker_stable() {
        let path = temporary_library_path("generic-gels-reopen");
        let first = FixtureLibrary::open(&path).unwrap();
        let expected = first.gel_catalog(GENERIC_GEL_CATALOG_ID).unwrap().unwrap();
        drop(first);

        let reopened = FixtureLibrary::open(&path).unwrap();
        assert_eq!(
            reopened
                .gel_catalog(GENERIC_GEL_CATALOG_ID)
                .unwrap()
                .unwrap(),
            expected
        );
        assert_eq!(
            reopened
                .conn
                .query_row(
                    "SELECT value FROM library_metadata WHERE key=?1",
                    [GENERIC_GEL_CATALOG_SEED_KEY],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            GENERIC_GEL_CATALOG_SEED_VERSION
        );

        drop(reopened);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn existing_installation_adds_the_generic_seed_without_mutating_operator_catalogs() {
        let path = temporary_library_path("generic-gels-migration");
        let operator_catalog_id = Uuid::new_v4();
        let operator_entry_id = Uuid::new_v4();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE gel_catalogs(id TEXT PRIMARY KEY,name TEXT NOT NULL,revision INTEGER NOT NULL);
                 CREATE TABLE gel_catalog_entries(catalog_id TEXT NOT NULL,entry_id TEXT NOT NULL,number TEXT NOT NULL,name TEXT NOT NULL,display_srgb TEXT NOT NULL,visualizer_srgb TEXT NOT NULL,sort_order INTEGER NOT NULL,PRIMARY KEY(catalog_id,entry_id),UNIQUE(catalog_id,number),FOREIGN KEY(catalog_id) REFERENCES gel_catalogs(id) ON DELETE CASCADE);",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO gel_catalogs(id,name,revision) VALUES(?1,'House catalog',7)",
                [operator_catalog_id.to_string()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO gel_catalog_entries(catalog_id,entry_id,number,name,display_srgb,visualizer_srgb,sort_order) VALUES(?1,?2,'H1','House blue','#123456','#102040',0)",
                params![operator_catalog_id.to_string(), operator_entry_id.to_string()],
            )
            .unwrap();
        drop(connection);

        let library = FixtureLibrary::open(&path).unwrap();
        assert!(
            library
                .gel_catalog(GENERIC_GEL_CATALOG_ID)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            library.gel_catalog(operator_catalog_id).unwrap().unwrap(),
            GelCatalog {
                id: operator_catalog_id,
                revision: 7,
                name: "House catalog".into(),
                entries: vec![GelCatalogEntry {
                    id: operator_entry_id,
                    number: "H1".into(),
                    name: "House blue".into(),
                    display_srgb: "#123456".into(),
                    visualizer_srgb: "#102040".into(),
                }],
            }
        );

        drop(library);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reopening_preserves_an_operator_modified_generic_catalog_revision() {
        let path = temporary_library_path("generic-gels-operator-revision");
        let library = FixtureLibrary::open(&path).unwrap();
        let csv = format!(
            "{HEADER}G01,Operator red,#AA1111,#BB1010\nG16,Operator green,#11AA11,#10BB10\n"
        );
        let preview = library
            .preview_gel_catalog_csv_import(
                GelCatalogImportTarget::Update {
                    catalog_id: GENERIC_GEL_CATALOG_ID,
                    expected_revision: 1,
                },
                "Operator generic gels",
                csv.as_bytes(),
            )
            .unwrap();
        let modified = library.confirm_gel_catalog_csv_import(&preview).unwrap();
        assert_eq!(modified.revision, 2);
        assert_eq!(modified.entries.len(), 17);
        assert_eq!(modified.entries[1].id, GENERIC_GEL_ENTRY_IDS[1]);
        drop(library);

        let reopened = FixtureLibrary::open(&path).unwrap();
        assert_eq!(
            reopened
                .gel_catalog(GENERIC_GEL_CATALOG_ID)
                .unwrap()
                .unwrap(),
            modified
        );

        drop(reopened);
        let _ = std::fs::remove_file(path);
    }
}
