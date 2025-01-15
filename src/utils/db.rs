use std::rc::Rc;

use rexie::{ObjectStore, Rexie, Store, Transaction, TransactionMode};
use serde_wasm_bindgen::from_value as serde_from_wasm;
use wasm_bindgen::JsValue;
use yew::AttrValue;

use crate::errors::Result;
use crate::models::{PageImage, PageOcr, Settings, VolumeMetadata};

const G: &str = "global";
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
        .version(2)
        .add_object_store(ObjectStore::new(G))
        .add_object_store(ObjectStore::new(V).key_path("id").auto_increment(true))
        .add_object_store(ObjectStore::new(P))
        .add_object_store(ObjectStore::new(O))
        .build()
        .await?;
    Ok(rexie)
}

pub async fn get_settings(db: &Rc<Rexie>) -> Result<Settings> {
    let settings = db.transaction(&[G], TransactionMode::ReadOnly)?
        .store(G)?
        .get(&JsValue::from_str("settings")).await
        .map(|value| serde_from_wasm(value).unwrap_or(Settings::default()))?;
    Ok(settings)
}

pub async fn put_settings(db: &Rc<Rexie>, settings: &Settings) -> Result<()> {
    let value = serde_wasm_bindgen::to_value(settings)?;
    db.transaction(&[G], TransactionMode::ReadWrite)?
        .store(G)?
        .put(&value, Some(&JsValue::from_str("settings"))).await?;
    Ok(())
}

/// Start a transaction with the `pages` and `ocr` stores for bulk insertion.
/// This method is just to keep all string references to the stores in this file.
pub fn start_bulk_write_txn(db: &Rc<Rexie>) -> Result<(Transaction, Store, Store)> {
    let txn = db.transaction(&[P, O], TransactionMode::ReadWrite)?;
    let pages = txn.store(P)?;
    let ocr = txn.store(O)?;
    Ok((txn, pages, ocr))
}

#[allow(dead_code)]
pub async fn get_page(db: Rc<Rexie>, volume_id: u32, name: AttrValue) -> Result<PageImage> {
    let key = js_sys::Array::of2(&volume_id.into(), &name.as_str().into());
    let txn = db.transaction(&[P], TransactionMode::ReadOnly)?;
    let pages = txn.store(P)?;
    Ok(pages.get(&key).await?.into())
}

pub async fn put_ocr(db: &Rc<Rexie>, ocr: &PageOcr, key: &JsValue) -> Result<()> {
    let value = serde_wasm_bindgen::to_value(ocr)?;
    let txn = db.transaction(&[O], TransactionMode::ReadWrite)?;
    txn.store(O)?.put(&value, Some(key)).await?;
    Ok(())
}

/// The associated rows from `pages` and `ocr` share the same key.
pub async fn get_page_and_ocr(db: &Rc<Rexie>, key: &JsValue) -> Result<(PageImage, PageOcr)> {
    let txn = db.transaction(&[P, O], TransactionMode::ReadOnly)?;
    let pages = txn.store(P)?;
    let page_value: PageImage = pages.get(key).await?.into();

    let ocr = txn.store(O)?;
    let ocr_value = ocr.get(key).await?;
    Ok((page_value, serde_wasm_bindgen::from_value(ocr_value)?))
}

pub async fn get_volume(db: &Rc<Rexie>, volume_id: u32) -> Result<VolumeMetadata> {
    let value = db.transaction(&[V], TransactionMode::ReadOnly)?
        .store(V)?
        .get(&volume_id.into()).await?;
    Ok(serde_from_wasm(value)?)
}

#[allow(dead_code)]
pub async fn get_all_volumes(db: Rc<Rexie>) -> Result<Vec<VolumeMetadata>> {
    let values = db.transaction(&[V], TransactionMode::ReadOnly)?
        .store(V)?
        .get_all(None, None, None, None).await?;
    Ok(values.into_iter().filter_map(|(_k, v)| serde_from_wasm(v).ok()).collect())
}

pub async fn get_all_volumes_with_covers(db: &Rc<Rexie>) -> Result<Vec<(VolumeMetadata, PageImage)>> {
    let txn = db.transaction(&[V, P], TransactionMode::ReadOnly)?;
    let values = txn.store(V)?.get_all(None, None, None, None).await?;
    let pages = txn.store(P)?;

    let mut result = Vec::with_capacity(values.len());
    for (_k, v) in values.into_iter() {
        let volume: VolumeMetadata = serde_from_wasm(v)?;
        let key = js_sys::Array::of2(&volume.id.unwrap().into(), &volume.cover().as_str().into());
        let cover: PageImage = pages.get(&key).await?.into();
        result.push((volume, cover));
    }
    Ok(result)
}


/// put_config inserts/updates a row within the "volumes" ObjectStore.
/// If `volume.id` is set, the object is updated.
pub async fn put_volume(db: &Rc<Rexie>, volume: &VolumeMetadata) -> Result<u32> {
    let config = serde_wasm_bindgen::to_value(volume)?;
    let txn = db.transaction(&[V], TransactionMode::ReadWrite)?;
    let volume_id = txn.store(V)?.put(&config, None).await?;
    txn.done().await?;
    Ok(volume_id.as_f64().unwrap() as u32)
}

/// delete_volume cascade deletes the volume with matching volume_id,
///   removing all images and ocr data.
pub async fn delete_volume(db: &Rc<Rexie>, volume_id: u32) -> Result<()> {
    let volume = get_volume(db, volume_id).await?;
    let txn = db.transaction(&[V, O, P], TransactionMode::ReadWrite)?;
    let id = volume_id.into();
    for (page_name, _) in volume.pages.iter() {
        let key = js_sys::Array::of2(&id, &page_name.as_str().into());
        txn.store(P)?.delete(&key).await?;
        txn.store(O)?.delete(&key).await?;
    }
    txn.store(V)?.delete(&id).await?;
    txn.done().await?;
    Ok(())
}
