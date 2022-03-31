// SPDX-FileCopyrightText: 2022 Hewlett-Packard Development Company, L.P.
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{env, fs, io};

use crate::{
    api::Api,
    db::DB,
    event::{DataCollectionPurpose, DeviceOSIds},
};

use hp_vendor_client::PurposesOutput;

// TODO: return parsable error?
fn get_purposes_from_api(
    os_install_id: String,
    locale: &str,
) -> Option<Vec<DataCollectionPurpose>> {
    let ids = DeviceOSIds::new(os_install_id).ok()?;
    let api = Api::new(ids).ok()?;
    api.purposes(Some(locale)).ok()
}

pub fn purposes() {
    let locale = env::args().skip(1).next().unwrap();

    let db = DB::open().unwrap();

    let opted = db.get_opted().unwrap();

    let purposes = db.get_purposes(Some(&locale)).unwrap();
    let purposes = if purposes.is_empty() {
        eprintln!(
            "No purposes for locale '{}'. Requesting from server.",
            locale
        );
        let os_install_id = db.get_os_install_id().unwrap();
        let purposes = get_purposes_from_api(os_install_id, &locale);
        if let Some(purposes) = &purposes {
            db.set_purposes(&locale, purposes).unwrap();
        }
        purposes
    } else {
        Some(purposes)
    };

    serde_json::to_writer(io::stdout(), &PurposesOutput { opted, purposes }).unwrap();
}

pub fn update_purposes() {
    let db = DB::open().unwrap();
    let os_install_id = db.get_os_install_id().unwrap();
    let ids = DeviceOSIds::new(os_install_id).ok().unwrap();
    let api = Api::new(ids).ok().unwrap();

    let purposes = api.purposes(None).unwrap();

    let file = fs::File::create("purposes.json").unwrap();
    serde_json::to_writer_pretty(file, &purposes).unwrap();

    eprintln!("Purposes written to `purposes.json`.");
}
