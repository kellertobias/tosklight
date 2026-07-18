use super::model::{MatterIdentity, MatterPairingData};
use rand::RngCore;
use rs_matter::BasicCommData;
use rs_matter::dm::devices::test::{TEST_PID, TEST_VID};
use rs_matter::error::Error;
use rs_matter::pairing::qr::{CommFlowType, QrPayload, no_optional_data};
use rs_matter::pairing::{DiscoveryCapabilities, qr::NoOptionalData};
use rs_matter::sc::pase::{Spake2pVerifierPassword, Spake2pVerifierPasswordRef};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::Path;

pub(super) const IDENTITY_FILE: &str = "identity.json";

pub(super) fn basic_info(
    identity: &MatterIdentity,
) -> rs_matter::dm::clusters::basic_info::BasicInfoConfig<'_> {
    rs_matter::dm::clusters::basic_info::BasicInfoConfig {
        vendor_name: "ToskLight",
        vid: TEST_VID,
        product_name: "ToskLight Matter Bridge",
        pid: TEST_PID,
        hw_ver: 1,
        hw_ver_str: "1",
        sw_ver: 1,
        sw_ver_str: env!("CARGO_PKG_VERSION"),
        serial_no: &identity.serial,
        unique_id: &identity.serial,
        device_name: "ToskLight",
        ..rs_matter::dm::clusters::basic_info::BasicInfoConfig::new()
    }
}

pub(super) fn commissioning_data(identity: &MatterIdentity) -> BasicCommData {
    let passcode = identity.passcode.to_le_bytes();
    BasicCommData {
        password: Spake2pVerifierPassword::new_from_ref(Spake2pVerifierPasswordRef::new(&passcode)),
        discriminator: identity.discriminator,
    }
}

pub(super) fn pairing_data(identity: &MatterIdentity) -> Result<MatterPairingData, Error> {
    let commissioning = commissioning_data(identity);
    let qr = QrPayload::new(
        DiscoveryCapabilities::IP,
        CommFlowType::Standard,
        commissioning.clone(),
        TEST_VID,
        TEST_PID,
        &identity.serial,
        no_optional_data as NoOptionalData,
    );
    let mut buffer = [0_u8; 512];
    let (qr_code, _) = qr.as_str(&mut buffer)?;
    Ok(MatterPairingData {
        qr_code: qr_code.to_owned(),
        manual_code: commissioning.compute_pretty_pairing_code().to_string(),
        discriminator: identity.discriminator,
    })
}

pub(super) fn load_or_create_identity(storage_dir: &Path) -> io::Result<MatterIdentity> {
    fs::create_dir_all(storage_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(storage_dir, fs::Permissions::from_mode(0o700))?;
    }
    let path = storage_dir.join(IDENTITY_FILE);
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let identity = random_identity();
            write_private_json(&path, &identity)?;
            Ok(identity)
        }
        Err(error) => Err(error),
    }
}

fn random_identity() -> MatterIdentity {
    let mut random = rand::thread_rng();
    let passcode = loop {
        let candidate = random.next_u32() % 99_999_998 + 1;
        if !matches!(
            candidate,
            11_111_111
                | 22_222_222
                | 33_333_333
                | 44_444_444
                | 55_555_555
                | 66_666_666
                | 77_777_777
                | 88_888_888
                | 12_345_678
                | 87_654_321
        ) {
            break candidate;
        }
    };
    MatterIdentity {
        passcode,
        discriminator: (random.next_u32() & 0x0fff) as u16,
        serial: format!("{:016x}", random.next_u64()),
    }
}

fn write_private_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(temporary, path)
}
