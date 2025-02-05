use std::io::{Cursor, Read, Seek, Write};
use std::rc::Rc;

use gloo_file::futures::read_as_bytes as gloo_file_read;
use rexie::Rexie;
use zip::{read::ZipArchive, result::ZipError, write::{SimpleFileOptions, ZipWriter}};

use crate::models::{PageImage, PageOcr, VolumeId, VolumeMetadata};
use crate::utils::db::{get_page_and_ocr, get_settings, get_volume, start_bulk_write_txn};

const METADATA_FILE: &str = "mokuro-metadata.json";

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/utils/archive.js")]
extern "C" {
    type Archive;

    #[wasm_bindgen(constructor)]
    fn new() -> Archive;

    #[wasm_bindgen(method)]
    async fn load(this: &Archive, file: web_sys::File);

    #[wasm_bindgen(catch, method)]
    async fn file(this: &Archive, name: &str) -> Result<JsValue, JsValue>;
}

/// extract a zip archive in memory and inserts the data into the mokuro IndexedDB.
pub async fn extract_ziparchive(
    db: &Rc<Rexie>, file: web_sys::File,
) -> crate::Result<(VolumeMetadata, gloo_file::ObjectUrl)> {
    let global_settings = get_settings(db).await?;
    let archive = Archive::new();
    archive.load(file).await;

    let mut volume = {
        let mut volume = {
            let data = js_to_u8_vec(archive.file(METADATA_FILE).await?);
            serde_json::from_slice::<VolumeMetadata>(&data)?
        };
        volume.id = 0;  // ensure id is not specified. IndexDB determines this.
        volume.magnifier = global_settings.magnifier;
        volume
    };

    let cover = volume.cover();
    let cover_object_url = {
        let cover_data = js_to_u8_vec(archive.file(cover).await?);
        PageImage::new(cover, &cover_data[..]).into()
    };

    let mut page_ocr_data = Vec::with_capacity(volume.pages.len());
    for (page_name, ocr_name) in volume.pages.iter() {
        let image_data = {
            let image_data = js_to_u8_vec(archive.file(page_name).await?);
            PageImage::new(page_name, &image_data[..])
        };

        let page_ocr = {
            let data = js_to_u8_vec(archive.file(ocr_name).await?);
            serde_json::from_slice::<PageOcr>(&data)?
        };
        page_ocr_data.push((page_name, image_data, page_ocr));
    }

    let (txn, volumes_store, pages_store, ocr_store) = start_bulk_write_txn(db)?;
    volume.id = {
        let config = serde_wasm_bindgen::to_value(&volume)?;
        let key = volumes_store.put(&config, None).await?;
        key.unchecked_into_f64() as VolumeId
    };
    let id = volume.id.into();
    for (name, image, ocr) in page_ocr_data {
        let key = js_sys::Array::of2(&id, &name.as_str().into());
        pages_store.add(image.as_ref(), Some(&key)).await?;
        let ocr_data = serde_wasm_bindgen::to_value(&ocr)?;
        ocr_store.add(&ocr_data, Some(&key)).await?;
    }
    txn.commit().await?;
    Ok((volume, cover_object_url))
}

/// construct a zip archive in memory from the volume data stored in the
/// mokuro IndexedDB. The resultant gloo_file::File is a JS object that
/// can then be downloaded through the browser.
pub async fn create_ziparchive(
    db: Rc<Rexie>, volume_id: VolumeId,
) -> crate::Result<gloo_file::File> {
    let volume: VolumeMetadata = get_volume(&db, volume_id).await?;

    let mut archive = ZipWriter::new(Cursor::new(vec![]));
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    let metadata = {
        let mut volume = volume.clone();
        volume.id = 0;
        serde_json::to_vec(&volume)?
    };
    write_zipfile(&mut archive, METADATA_FILE, &metadata, options)?;
    archive.add_directory("_ocr/", options)?;

    let id = volume.id.into();
    for (page_name, ocr_name) in volume.pages.iter() {
        let key = js_sys::Array::of2(&id, &page_name.as_str().into());
        let (image, ocr) = get_page_and_ocr(&db.clone(), &key.into()).await?;

        let image_data = gloo_file_read(image.as_ref()).await?;
        write_zipfile(&mut archive, page_name, &image_data, options)?;

        let ocr_data = serde_json::to_vec(&ocr)?;
        write_zipfile(&mut archive, ocr_name, &ocr_data, options)?;
    }

    let name = &format!("{}.mbz.zip", volume.title);
    let buffer = archive.finish()?.into_inner();
    Ok(gloo_file::File::new(name, &buffer[..]))
}

fn read_zipfile<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> crate::Result<Vec<u8>> {
    // construct the ZipFile object.
    let mut file = archive.by_name(name).map_err(|err| {
        if let ZipError::FileNotFound = err {
            return crate::errors::AppError::InvalidMokuroFile(
                crate::errors::InvalidMokuroFileError::MissingFile(name.to_string())
            );
        }
        err.into()
    })?;

    // copy the contents of the ZipFile into a Vec<u8>
    let mut buffer = Vec::with_capacity(file.size() as usize);
    std::io::copy(&mut file, &mut buffer).map_err(ZipError::Io)?;
    Ok(buffer)
}

fn write_zipfile<W: Write + Seek>(
    writer: &mut ZipWriter<W>,
    name: &str,
    content: &[u8],
    options: SimpleFileOptions,
) -> zip::result::ZipResult<usize> {
    writer.start_file(name, options)?;
    let mut bytes_written = 0;
    while bytes_written < content.len() {
        bytes_written += writer.write(&content[bytes_written..])?;
    }
    Ok(bytes_written)
}


#[inline(always)]
fn js_to_u8_vec(data: JsValue) -> Vec<u8> {
    js_sys::Uint8Array::new(&data).to_vec()
}

// pub fn get_zipfile<'z, R: Read + Seek>(
//     archive: &'z mut ZipArchive<R>,
//     name: &str,
// ) -> crate::Result<ZipFile<'z>> {
//     archive.by_name(name).map_err(|err| {
//         if let ZipError::FileNotFound = err {
//             return crate::errors::AppError::InvalidMokuroFile(
//                 crate::errors::InvalidMokuroFileError::MissingFile(name.to_string())
//             );
//         }
//         err.into()
//     })
// }

// pub fn read_zipfile(mut file: ZipFile) -> Vec<u8> {
//     let mut buffer = Vec::with_capacity(file.size() as usize);
//     std::io::copy(&mut file, &mut buffer).expect_throw("failed to read data from zip archive");
//     buffer
// }