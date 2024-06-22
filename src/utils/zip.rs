use std::io::{Cursor, Read, Seek, Write};
use std::rc::Rc;

use gloo_file::futures::read_as_bytes as gloo_file_read;
use rexie::Rexie;
use wasm_bindgen::UnwrapThrowExt;
use zip::{read::ZipArchive, result::ZipError, write::{SimpleFileOptions, ZipWriter}};

use crate::models::{PageImage, PageOcr, VolumeMetadata};
use crate::utils::db::{get_page_and_ocr, get_volume, put_volume, start_bulk_write_txn};

/// extract a zip archive in memory and inserts the data into the mokuro IndexedDB.
pub async fn extract_ziparchive(
    db: Rc<Rexie>, file_obj: gloo_file::File,
) -> crate::Result<(VolumeMetadata, gloo_file::ObjectUrl)> {
    let mut archive = {
        let reader = Cursor::new(gloo_file_read(&file_obj).await?);
        ZipArchive::new(reader)?
    };

    let volume = {
        let mut volume = {
            let data = read_zipfile(&mut archive, "mokuro.json")?;
            serde_json::from_slice::<VolumeMetadata>(&data).unwrap()
        };
        put_volume(&db, &mut volume).await?;
        volume
    };

    let cover = volume.cover();
    let cover_object_url = {
        let cover_data = read_zipfile(&mut archive, cover)?;
        PageImage::new(cover, &cover_data[..]).into()
    };

    let id = volume.id.unwrap().into();
    let (txn, pages_store, ocr_store) = start_bulk_write_txn(&db)?;
    for (page_name, ocr_name) in volume.pages.iter() {
        let key = js_sys::Array::of2(&id, &page_name.as_str().into());
        let image_data = {
            let image_data = read_zipfile(&mut archive, page_name)?;
            PageImage::new(page_name, &image_data[..])
        };
        pages_store.add(image_data.as_ref(), Some(&key)).await?;

        let page_ocr = {
            let ocr_data = read_zipfile(&mut archive, ocr_name)?;
            let ocr = serde_json::from_slice::<PageOcr>(&ocr_data).unwrap();
            serde_wasm_bindgen::to_value(&ocr).unwrap()
        };
        ocr_store.add(&page_ocr, Some(&key)).await?;
    }

    txn.commit().await?;
    Ok((volume, cover_object_url))
}

/// construct a zip archive in memory from the volume data stored in the
/// mokuro IndexedDB. The resultant gloo_file::File is a JS object that
/// can then be downloaded through the browser.
pub async fn create_ziparchive(
    db: Rc<Rexie>, volume_id: u32,
) -> crate::Result<gloo_file::File> {
    let volume: VolumeMetadata = get_volume(db.clone(), volume_id).await?;

    let mut archive = ZipWriter::new(Cursor::new(vec![]));
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    let metadata = serde_json::to_vec(&volume).unwrap();
    write_zipfile(&mut archive, "mokuro-metadata.json", &metadata, options)
        .expect_throw("failed to write mokuro-config.json to zip archive");
    archive.add_directory("_ocr/", options).unwrap();

    let id = volume.id.unwrap().into();
    for (page_name, ocr_name) in volume.pages.iter() {
        let key = js_sys::Array::of2(&id, &page_name.as_str().into());
        let (image, ocr) = get_page_and_ocr(db.clone(), key.into()).await?;

        let image_data = gloo_file_read(image.as_ref()).await
            .expect_throw("failed to convert Blob to Vec<u8>");
        write_zipfile(&mut archive, page_name, &image_data, options)
            .expect_throw("failed to write image file to zip archive");

        let ocr_data = serde_json::to_vec(&ocr).unwrap();
        write_zipfile(&mut archive, ocr_name, &ocr_data, options)
            .expect_throw("failed to write image file to zip archive");
    }

    let buffer = archive.finish().unwrap().into_inner();
    Ok(gloo_file::File::new("test.zip", &buffer[..]))
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
    std::io::copy(&mut file, &mut buffer).expect_throw("failed to read data from zip archive");
    Ok(buffer)
}

fn write_zipfile<W: Write + Seek>(
    writer: &mut ZipWriter<W>,
    name: &str,
    content: &[u8],
    options: SimpleFileOptions,
) -> std::io::Result<usize> {
    writer.start_file(name, options).unwrap();
    let mut bytes_written = 0;
    while bytes_written < content.len() {
        bytes_written += writer.write(&content[bytes_written..])?;
    }
    Ok(bytes_written)
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