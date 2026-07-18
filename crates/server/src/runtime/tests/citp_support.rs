fn citp_test_packet(content: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = 26 + payload.len();
    let mut bytes = Vec::with_capacity(size);
    bytes.extend_from_slice(b"CITP");
    bytes.extend_from_slice(&[1, 0]);
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&(size as u32).to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(b"MSEX");
    bytes.extend_from_slice(&[1, 2]);
    bytes.extend_from_slice(&content);
    bytes.extend_from_slice(payload);
    bytes
}
async fn read_citp_test_packet(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
    use tokio::io::AsyncReadExt;
    let mut header = [0_u8; 20];
    stream.read_exact(&mut header).await.unwrap();
    let size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    let mut packet = header.to_vec();
    packet.resize(size, 0);
    stream.read_exact(&mut packet[20..]).await.unwrap();
    packet
}

fn schema_v1_dimmer_rows(
    manufacturer: &str,
    family: &str,
) -> Vec<(light_fixture::FixtureDefinition, Vec<u8>)> {
    [1_u16, 2_u16]
        .into_iter()
        .enumerate()
        .map(|(index, footprint)| {
            let definition = light_fixture::FixtureDefinition {
                schema_version: 1,
                id: light_core::FixtureId::new(),
                revision: 1,
                manufacturer: manufacturer.into(),
                device_type: "dimmer".into(),
                name: family.into(),
                model: family.into(),
                mode: if index == 0 { "Coarse" } else { "Fine" }.into(),
                footprint,
                heads: vec![light_fixture::LogicalHead {
                    index: 0,
                    name: "Main".into(),
                    shared: true,
                    parameters: vec![light_fixture::Parameter {
                        attribute: light_core::AttributeKey("intensity".into()),
                        components: (0..footprint)
                            .map(|offset| light_fixture::ChannelComponent {
                                offset,
                                byte_order: light_fixture::ByteOrder::MsbFirst,
                            })
                            .collect(),
                        default: 0.0,
                        virtual_dimmer: false,
                        metadata: light_fixture::ParameterMetadata::default(),
                        capabilities: Vec::new(),
                    }],
                }],
                color_calibration: None,
                physical: light_fixture::FixturePhysicalProperties::default(),
                model_asset: None,
                icon_asset: None,
                hazardous: false,
                direct_control_protocols: Vec::new(),
                signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
                safe_values: BTreeMap::new(),
                profile_id: None,
                mode_id: None,
                profile_snapshot: None,
            };
            (
                definition,
                format!("retained-startup-gdtf-{index}").into_bytes(),
            )
        })
        .collect()
}

fn seed_schema_v1_fixture_database(
    data_dir: &FsPath,
    rows: &[(light_fixture::FixtureDefinition, Vec<u8>)],
) {
    std::fs::create_dir_all(data_dir).unwrap();
    let connection = rusqlite::Connection::open(data_dir.join("fixtures.sqlite")).unwrap();
    connection.execute_batch("CREATE TABLE fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));").unwrap();
    for (definition, source) in rows {
        connection.execute(
                "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![
                    definition.id.0.to_string(),
                    definition.revision,
                    definition.manufacturer,
                    definition.model,
                    definition.mode,
                    serde_json::to_string(definition).unwrap(),
                    source,
                ],
            ).unwrap();
    }
}
