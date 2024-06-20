use std::rc::Rc;

use rexie::Rexie;
use web_sys::{DragEvent, Event, FileList, HtmlInputElement, MouseEvent};
use yew::{Callback, function_component, html, Html, HtmlResult, TargetCast, use_state};
use yew::suspense::{Suspense, use_future_with};
use yew_autoprops::autoprops;

use crate::utils::timestamp as new_timestamp;
use crate::utils::web::{ask_to_persist_storage, is_web_storage_persisted};
use crate::utils::zip::{create_ziparchive, extract_ziparchive};

/// upload_modal creates a modal overlay where users can upload zip archives.
/// It tries to check if the user has enabled persisted storage for the site,
///   and if not prompts the user to enable it.
#[autoprops]
#[function_component(UploadModal)]
pub fn upload_modal(
    db: &Rc<Rexie>,
    close_modal: &Callback<MouseEvent>,
    timestamp: u64,
) -> HtmlResult {
    // `persist` asks the browser whether the user has enabled persisted storage.
    let future = use_future_with(timestamp, |_| is_web_storage_persisted())?;
    let persisted = future.as_ref();

    gloo_console::debug!(format!("storage persisted: {} {:?}", timestamp, persisted));

    let persist_storage = Callback::from(|_: MouseEvent| {
        wasm_bindgen_futures::spawn_local(async {
            // We don't care about the result, we just initiate the request.
            let _ = ask_to_persist_storage().await;
        })
    });

    let file_names = use_state(Vec::<gloo_file::File>::new);
    let onchange = {
        let file_names = file_names.clone();
        Callback::from(move |event: Event| {
            let input: HtmlInputElement = event.target_unchecked_into();
            file_names.set(upload_files(input.files()));
        })
    };
    let ondrop = {
        let file_names = file_names.clone();
        Callback::from(move |event: DragEvent| {
            event.prevent_default();
            let file_list = event.data_transfer().unwrap().files();
            file_names.set(upload_files(file_list));
        })
    };
    let cancel_click = Callback::from(|e: MouseEvent| e.stop_propagation());
    let cancel_drag = Callback::from(|e: DragEvent| e.prevent_default());
    Ok(html! {
        <div class="modal" onclick={close_modal}>
            <div class="modal-content" onclick={cancel_click}>
                if let Ok(false) = persisted {
                    <p>
                        {"please persist storage to protect your files --->"}
                        <button onclick={persist_storage}>{"persist storage"}</button>
                    </p>
                }
                <p id="title">{ "Upload Your Files To The Cloud" }</p>
                <label for="file-upload">
                    <div id="drop-container" ondrop={ondrop} ondragover={&cancel_drag} ondragenter={&cancel_drag}>
                        <i class="fa fa-cloud-upload"></i>
                        <p>{"Drop your images here or click to select"}</p>
                    </div>
                </label>
                <input id="file-upload" type="file" accept="application/zip" multiple={true} {onchange}/>
                if let Some(file) = file_names.first() {
                    <Suspense fallback={html! {<div>{"Processing..."}</div>}}>
                        <Preview {db} file_obj={file.clone()} timestamp={new_timestamp()}/>
                    </Suspense>
                }
            </div>
        </div>
    })
}

#[autoprops]
#[function_component(Preview)]
fn preview(db: &Rc<Rexie>, file_obj: &gloo_file::File, timestamp: u64) -> HtmlResult {
    let future = use_future_with(timestamp, |_| {
        extract_ziparchive(db.clone(), file_obj.clone())
    })?;
    let (volume, cover_object_url) = future.as_ref().unwrap();

    Ok(html! {
        <div id="preview-area">
            <DownloadButton {db} volume_id={volume.id.unwrap()}/>
            <img id="ItemPreview" src={cover_object_url.to_string()}/>
            { format!("volume_id: {}", volume.id.unwrap())}
        </div>
    })
}

#[autoprops]
#[function_component(DownloadButton)]
fn download_button(db: &Rc<Rexie>, volume_id: &u32) -> Html {
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
                <DownloadButtonInner {db} {volume_id}/>
            </Suspense>
        }
    }
}

#[autoprops]
#[function_component(DownloadButtonInner)]
fn download_button_inner(db: &Rc<Rexie>, volume_id: &u32) -> HtmlResult {
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

fn upload_files(files: Option<FileList>) -> Vec<gloo_file::File> {
    let mut result = Vec::new();
    if let Some(files) = files {
        let files = js_sys::try_iter(&files)
            .unwrap()
            .unwrap()
            .map(|v| web_sys::File::from(v.unwrap()))
            .map(gloo_file::File::from);
        result.extend(files);
    }
    result
}