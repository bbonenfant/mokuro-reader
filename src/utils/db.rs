use std::rc::Rc;

use rexie::{ObjectStore, Rexie, Store, Transaction, TransactionMode};
use wasm_bindgen::JsValue;

use crate::errors::Result;
use crate::models::{PageImage, PageOcr, VolumeMetadata};

const O: &str = "ocr";
const P: &str = "pages";
const V: &str = "volumes";


/// Creates the IndexedDB instance used by this App.
/// There are three ObjectStores: volumes, pages, & ocr.
///   - `volumes` contains JSON data and is the contents of the
///     mokuro-metadata.json file within the zip archive.
///     It's indexed by auto-incremented keys, meaning that multiple
///     versions of the same zip archive can be uploaded.
///     Each row contains an array of the names of the image and ocr files.
///   - `pages` contains raw binary blobs of the image files.
///     Each row is keyed by (volume_id, page_name).
///   - `ocr` contains the contents of the JSON files contained
///     within the _ocr directory of the zip archive.
///     Each row is keyed by (volume_id, page_name), where page_name
///     is the associated image for the ocr output.
///
/// Note: `pages` and `ocr` were into separate stores because the rows
///       of `pages` will never change, but `ocr` might be updated.
///       IndexedDB does not support partial updates.
pub async fn create_database() -> rexie::Result<Rexie> {
    let rexie = Rexie::builder("mokuro")
        .version(1)
        .add_object_store(ObjectStore::new(V).key_path("id").auto_increment(true))
        .add_object_store(ObjectStore::new(P))
        .add_object_store(ObjectStore::new(O))
        .build()
        .await?;
    Ok(rexie)
}

/// Start a transaction with the `pages` and `ocr` stores for bulk insertion.
/// This method is just to keep all string references to the stores in this file.
pub fn start_bulk_write_txn(db: &Rc<Rexie>) -> Result<(Transaction, Store, Store)> {
    let txn = db.transaction(&[P, O], TransactionMode::ReadWrite)?;
    let pages = txn.store(P)?;
    let ocr = txn.store(O)?;
    Ok((txn, pages, ocr))
}

/// The associated rows from `pages` and `ocr` share the same key.
pub async fn get_page_and_ocr(db: &Rc<Rexie>, key: &JsValue) -> Result<(PageImage, PageOcr)> {
    let txn = db.transaction(&[P, O], TransactionMode::ReadOnly)?;
    let pages = txn.store(P)?;
    let page_value: PageImage = pages.get(key).await?.into();

    let ocr = txn.store(O)?;
    let ocr_value = ocr.get(key).await?;
    Ok((page_value, serde_wasm_bindgen::from_value(ocr_value).unwrap()))
}

pub async fn get_volume(db: &Rc<Rexie>, volume_id: u32) -> Result<VolumeMetadata> {
    let value = db.transaction(&[V], TransactionMode::ReadOnly)?
        .store(V)?
        .get(&volume_id.into()).await?;
    Ok(serde_wasm_bindgen::from_value(value).unwrap())
}

/// put_config inserts/updates a row within the "volumes" ObjectStore.
/// If `volume.id` is set, the object is updated.
pub async fn put_volume(db: &Rc<Rexie>, volume: &mut VolumeMetadata) -> Result<u32> {
    let config = serde_wasm_bindgen::to_value(volume).unwrap();
    let key = volume.id.map(|k| JsValue::from_f64(k as f64));

    let txn = db.transaction(&[V], TransactionMode::ReadWrite)?;
    let volume_id = txn
        .store(V)?
        .put(&config, key.as_ref())
        .await?;
    txn.done().await?;

    let id = volume_id.as_f64().unwrap() as u32;
    volume.id = Some(id);
    Ok(id)
}