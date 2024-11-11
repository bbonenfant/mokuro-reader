use std::rc::Rc;

use rexie::Rexie;
use web_sys::{DragEvent, Event, FileList, HtmlInputElement, MouseEvent};
use yew::suspense::{use_future_with, Suspense};
use yew::{function_component, html, use_state, Callback, HtmlResult, TargetCast};
use yew_autoprops::autoprops;

use crate::utils::timestamp;
use crate::utils::web::{ask_to_persist_storage, is_web_storage_persisted};
use crate::utils::zip::extract_ziparchive;

/// upload_modal creates a modal overlay where users can upload zip archives.
/// It tries to check if the user has enabled persisted storage for the site,
///   and if not prompts the user to enable it.
#[autoprops]
#[function_component(UploadModal)]
pub fn upload_modal(
    db: &Rc<Rexie>,
    close_modal: &Callback<MouseEvent>,
    rerender: u64,
) -> HtmlResult {
    // `persist` asks the browser whether the user has enabled persisted storage.
    let future = use_future_with(rerender, |_| is_web_storage_persisted())?;
    let persisted = future.as_ref();

    gloo_console::debug!(format!("storage persisted: {} {:?}", rerender, persisted));

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
    let cancel_drag = &Callback::from(|e: DragEvent| e.prevent_default());
    Ok(html! {
        <div id="UploadModal" onclick={close_modal}>
            <div class="modal-content" onclick={cancel_click}>
                if let Ok(false) = persisted {
                    <p>
                        {"please persist storage to protect your files --->"}
                        <button onclick={persist_storage}>{"persist storage"}</button>
                    </p>
                }
                <p id="title">{ "Upload Your Mokuro Manga Files" }</p>
                <label for="file-upload">
                    <div id="drop-container" {ondrop} ondragover={cancel_drag} ondragenter={cancel_drag}>
                        <p>{"Drop your Mokuro files here or click to select"}</p>
                    </div>
                </label>
                <input id="file-upload" type="file" accept="application/zip" multiple={true} {onchange}/>
                if let Some(file) = file_names.first() {
                    <Suspense fallback={html! {<div>{"Processing..."}</div>}}>
                        <Preview {db} file_obj={file.clone()} rerender={timestamp()}/>
                    </Suspense>
                }
            </div>
        </div>
    })
}

#[autoprops]
#[function_component(Preview)]
fn preview(db: &Rc<Rexie>, file_obj: &gloo_file::File, rerender: u64) -> HtmlResult {
    let future = use_future_with(rerender, |_| {
        extract_ziparchive(db.clone(), file_obj.clone())
    })?;
    let (volume, cover_object_url) = future.as_ref().unwrap();

    Ok(html! {
        <div id="UploadPreview" class="flexbox">
            <img id="ItemPreview" src={cover_object_url.to_string()}/>
            { format!("volume_id: {}", volume.id.unwrap())}
        </div>
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