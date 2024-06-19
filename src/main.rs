use std::io::{Cursor, Write};
use std::rc::Rc;

use gloo_file::futures::read_as_bytes as gloo_file_read;
use gloo_file::File as GlooFile;
use js_sys::Array;
use rexie::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use web_sys::{DragEvent, Event, FileList, HtmlInputElement};
use yew::prelude::*;
use yew::suspense::use_future_with;
use yew_autoprops::autoprops;
use zip::write::SimpleFileOptions;

// web_sys::StorageManager::persist(&self);

#[function_component(App)]
fn app() -> Html {
    html! {
        <Suspense fallback={html! {<div>{"Initialing Database..."}</div>}}>
            <Main/>
        </Suspense>
    }
}

#[function_component(Main)]
fn main() -> HtmlResult {
    let db_future = use_future_with("", move |_| create_database())?;
    let db = db_future
        .as_ref()
        .expect("unable to initialize database")
        .clone();

    let file_names: UseStateHandle<Vec<GlooFile>> = use_state(Vec::new);
    let (fn_clone1, fn_clone2) = (file_names.clone(), file_names.clone());
    let onchange = Callback::from(move |event: Event| {
        let input: HtmlInputElement = event.target_unchecked_into();
        fn_clone1.set(upload_files(input.files()));
    });
    let ondrop = Callback::from(move |event: DragEvent| {
        event.prevent_default();
        let filelist = event.data_transfer().unwrap().files();
        fn_clone2.set(upload_files(filelist));
    });
    Ok(html! {
        <>
            <div id="wrapper">
                <p id="title">{ "Upload Your Files To The Cloud" }</p>
                <label for="file-upload">
                    <div
                        id="drop-container"
                        ondrop={ondrop}
                        ondragover={Callback::from(|event: DragEvent| {
                            event.prevent_default();
                        })}
                        ondragenter={Callback::from(|event: DragEvent| {
                            event.prevent_default();
                        })}
                    >
                        <i class="fa fa-cloud-upload"></i>
                        <p>{"Drop your images here or click to select"}</p>
                    </div>
                </label>
                <input
                    id="file-upload"
                    type="file"
                    accept="application/zip"
                    multiple={true}
                    onchange={onchange}
                />
                if let Some(file) = file_names.first() {
                    <Suspense fallback={html! {<div>{"Processing..."}</div>}}>
                        <Preview {db} file_obj={file.clone()}/>
                    </Suspense>
                }
            </div>
        </>
    })
}

#[autoprops]
#[function_component(Preview)]
fn preview(db: &Rexie, file_obj: &GlooFile) -> HtmlResult {
    let future = use_future_with(file_obj.name(), move |_| {
        extract_ziparchive(db.clone(), file_obj.clone())
    })?;
    let (volume_id, cover_objurl) = future.as_ref().unwrap();

    // let download_db = db.clone();
    // let download_volume_id = volume_id.clone();
    // let onclick = Callback::from(move |_| {
    //     let db_ = download_db.clone();
    //     wasm_bindgen_futures::spawn_local(async move {
    //         let file = create_ziparchive(db_, download_volume_id)
    //             .await
    //             .expect("failed to create ziparchive");
    //         let url = gloo_file::ObjectUrl::from(file);
    //         web_sys::window()
    //             .unwrap()
    //             .open_with_url(&url.to_string())
    //             .unwrap();
    //     });
    // });

    Ok(html! {
        <div id="preview-area">
            <DownloadButton db={db.clone()} {volume_id}/>
            <img id="ItemPreview" src={cover_objurl.to_string()}/>
            { format!("volume_id: {}", volume_id)}
            // { for file_names.iter() }
        </div>
    })
}

#[autoprops]
#[function_component(DownloadButton)]
fn download_button(db: &Rexie, volume_id: &u32) -> Html {
    let download_requested = use_state(|| false);
    let state = download_requested.clone();
    let onclick = Callback::from(move |_| {
        state.set(true);
    });
    if !*download_requested {
        html! {
            <button {onclick}>{"Prepare Download"}</button>
        }
    } else {
        html! {
            <Suspense fallback={html! {<button>{"Preparing..."}</button>}}>
                <DownloadButtonInner db={db.clone()} {volume_id}/>
            </Suspense>
        }
    }
}

#[autoprops]
#[function_component(DownloadButtonInner)]
fn download_button_inner(db: &Rexie, volume_id: &u32) -> HtmlResult {
    let future = use_future_with(*volume_id, move |_| {
        create_ziparchive(db.clone(), *volume_id)
    })?;
    let file = future.as_ref().unwrap();
    let url = use_state(|| gloo_file::ObjectUrl::from(file.clone()));
    Ok(html! {
        <a href={url.to_string()} download={file.name()}>
            <button>{"Download"}</button>
        </a>
    })
}

async fn extract_ziparchive(db: Rexie, file_obj: GlooFile) -> Result<(u32, gloo_file::ObjectUrl)> {
    let mut zip_file = {
        let reader = Cursor::new(
            gloo_file_read(&file_obj)
                .await
                .expect("failed to read uploaded file"),
        );
        zip::ZipArchive::new(reader).expect("failed to read zip archive")
    };

    let config = {
        let config_data = read_zipfile(
            zip_file
                .by_name("mokuro.json")
                .expect("failed to extract config"),
        );
        serde_json::from_slice::<Config>(&config_data).unwrap()
    };
    let volume_id = {
        let config = serde_wasm_bindgen::to_value(&config).unwrap();
        let txn = db.transaction(&["volumes"], TransactionMode::ReadWrite)?;
        let volume_id = txn.store("volumes")?.put(&config, None).await?;
        txn.done().await?;
        volume_id.as_f64().unwrap() as u32
    };

    let cover_objurl = {
        let cover_data = read_zipfile(
            zip_file
                .by_name("cover.avif")
                .expect("failed to extract cover"),
        );
        gloo_file::ObjectUrl::from(GlooFile::new("cover.avif", &cover_data[..]))
    };

    let txn = db.transaction(&["pages", "ocr"], TransactionMode::ReadWrite)?;
    let pages_store = txn.store("pages")?;
    let ocr_store = txn.store("ocr")?;
    // pages_store
    //     .add(
    //         // &gloo_file::Blob::new("test_data").as_ref(),
    //         &JsValue::from_str("test_data"),
    //         Some(&JsValue::from_str("key")),
    //     )
    //     .await?;

    for (page_name, ocr_name) in config.pages.iter() {
        let key = Array::of2(
            &JsValue::from_f64(volume_id as f64),
            &JsValue::from_str(page_name),
        );

        let image_data = {
            let image_data = read_zipfile(
                zip_file
                    .by_name(page_name)
                    .expect("failed to extract image"),
            );
            // js_sys::ArrayBuffer::from(&image_data[..])
            gloo_file::Blob::new(&image_data[..])
        };
        gloo_console::log!(page_name, image_data.size());
        pages_store.add(image_data.as_ref(), Some(&key)).await?;

        let page_ocr = {
            let ocr_data = read_zipfile(
                zip_file
                    .by_name(ocr_name)
                    .expect("failed to extract ocr file"),
            );
            let ocr = serde_json::from_slice::<Ocr>(&ocr_data).unwrap();
            serde_wasm_bindgen::to_value(&ocr).unwrap()
        };
        ocr_store.add(&page_ocr, Some(&key)).await?;
    }

    txn.commit().await?;
    Ok((volume_id, cover_objurl))
}

async fn create_ziparchive(db: Rexie, volume_id: u32) -> Result<GlooFile> {
    let config: Config = {
        let txn = db.transaction(&["volumes"], TransactionMode::ReadOnly)?;
        let volumes = txn.store("volumes")?;
        let config_data = volumes.get(&volume_id.into()).await?;
        serde_wasm_bindgen::from_value(config_data).unwrap()
    };

    let mut archive = zip::write::ZipWriter::new(Cursor::new(vec![]));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    write_zipfile(
        &mut archive,
        "mokuro-config.json",
        &serde_json::to_vec(&config).unwrap(),
        options,
    )
    .expect("failed to write mokuro-config.json to zip archive");
    archive.add_directory("_ocr/", options).unwrap();

    for (page_name, ocr_name) in config.pages.iter() {
        let key = Array::of2(
            &JsValue::from_f64(volume_id as f64),
            &JsValue::from_str(page_name),
        );

        let (image_data, ocr_data): (Vec<u8>, Ocr) = {
            let txn = db.transaction(&["pages", "ocr"], TransactionMode::ReadOnly)?;
            let pages = txn.store("pages")?;
            let image_data = pages.get(&key).await?;
            let image_blob: gloo_file::Blob = {
                let image_blob: web_sys::Blob = image_data.into();
                image_blob.into()
            };

            let ocr = txn.store("ocr")?;
            let ocr_data = ocr.get(&key).await?;
            (
                gloo_file_read(&image_blob)
                    .await
                    .expect("failed to convert Blob to Vec<u8>"),
                serde_wasm_bindgen::from_value(ocr_data).unwrap(),
            )
        };

        write_zipfile(&mut archive, page_name, &image_data[..], options)
            .expect("failed to write image file to zip archive");
        write_zipfile(
            &mut archive,
            ocr_name,
            &serde_json::to_vec(&ocr_data).unwrap(),
            options,
        )
        .expect("failed to write image file to zip archive");
        // archive
        //     .start_file(ocr_name.clone(), options)
        //     .expect("failed to create ocr file in zip archive");
        // archive
        //     .write(&serde_json::to_vec(&ocr_data).unwrap())
        //     .expect("failed to write to zip archive");
    }

    let buffer = archive
        .finish()
        .expect("failed to finish zip archive")
        .into_inner();
    Ok(GlooFile::new("test.zip", &buffer[..]))
}

async fn create_database() -> Result<Rexie> {
    // web_sys::window().unwrap().navigator().storage().persist();

    // Create a new database
    let rexie = Rexie::builder("mokuro")
        // Set the version of the database to 1.0
        .version(1)
        // Add an object store named `employees`
        .add_object_store(
            ObjectStore::new("volumes")
                // Set the key path to `id`
                .key_path("id")
                // Enable auto increment
                .auto_increment(true)
                // Add an index named `email` with the key path `email` with unique enabled
                .add_index(Index::new("uuid", "volume_uuid")),
        )
        .add_object_store(
            ObjectStore::new("pages"), // .key_path("id")
                                       // .auto_increment(true)
                                       // .add_index(Index::new_array("volume_page", ["volume_id", "name"])),
        )
        .add_object_store(ObjectStore::new("ocr"))
        // Build the database
        .build()
        .await?;

    // // Check basic details of the database
    // assert_eq!(rexie.name(), "test");
    // assert_eq!(rexie.version(), 1.0);
    // assert_eq!(rexie.store_names(), vec!["employees"]);
    Ok(rexie)
}

/// put_config inserts/updates a config stored within the "volumes" ObjectStore.
/// This is intended to be used with the use_future_with hook, which is why
///   it has a single Rc argument. If Key is provided and the key exists within
///   the ObjectStore, the object is updated.
///
/// Example:
///   let txn_future = use_future_with((db.clone(), config, None), put_config)?;
async fn put_config(deps: Rc<(Rexie, Config, Option<u32>)>) -> Result<u32> {
    let (db, config, key) = &*deps;
    let config = serde_wasm_bindgen::to_value(&config).unwrap();
    let key = key.map(|k| JsValue::from_f64(k as f64));

    let transaction = db.transaction(&["volumes"], TransactionMode::ReadWrite)?;
    let volume_id = transaction
        .store("volumes")?
        .put(&config, key.as_ref())
        .await?;
    transaction.done().await?;
    Ok(volume_id.as_f64().unwrap() as u32)
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Config {
    #[serde(skip_serializing)]
    id: Option<u32>,
    version: String,
    created_at: String,
    modified_at: String,
    title: String,
    volume: String,
    volume_uuid: String,
    // Pages is an array of (page_name, ocr_name) pairs.
    pages: Box<[(String, String)]>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Ocr {
    img_width: u32,
    img_height: u32,
    blocks: Vec<Block>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Block {
    #[serde(rename = "box")]
    box_: (u32, u32, u32, u32),
    vertical: bool,
    font_size: f32,
    lines: Vec<String>,
    // lines_coords: Vec<Vec<(f32, f32)>>,
}

fn upload_files(files: Option<FileList>) -> Vec<GlooFile> {
    let mut result = Vec::new();
    if let Some(files) = files {
        let files = js_sys::try_iter(&files)
            .unwrap()
            .unwrap()
            .map(|v| web_sys::File::from(v.unwrap()))
            .map(GlooFile::from);
        result.extend(files);
    }
    result
}

pub fn read_zipfile(mut file: zip::read::ZipFile) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(file.size() as usize);
    std::io::copy(&mut file, &mut buffer).expect("failed to read file from ZipArchive into buffer");
    buffer
}

pub fn write_zipfile<W: Write + std::io::Seek>(
    writer: &mut zip::write::ZipWriter<W>,
    name: &str,
    content: &[u8],
    options: zip::write::SimpleFileOptions,
) -> std::io::Result<usize> {
    writer.start_file(name, options).unwrap();
    let mut bytes_written = 0;
    while bytes_written < content.len() {
        bytes_written += writer.write(&content[bytes_written..])?;
    }
    Ok(bytes_written)
}

fn main() {
    yew::Renderer::<App>::new().render();
}
