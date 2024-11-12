use enclose::enclose;
use rexie::Rexie;
use std::rc::Rc;
use wasm_bindgen::UnwrapThrowExt;
use web_sys::{DragEvent, Event, FileList, HtmlInputElement, MouseEvent};
use yew::{html, AttrValue, Callback, Component, Context, Html, Properties, TargetCast};

use crate::models::VolumeMetadata;
use crate::utils::web::{ask_to_persist_storage, is_web_storage_persisted};
use crate::utils::zip::extract_ziparchive;

pub struct ExtractionError {
    error: crate::errors::AppError,
    filename: String,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub db: Rc<Rexie>,
    pub close_modal: Callback<MouseEvent>,
}

pub enum Message {
    Prompt,
    Process(Vec<gloo_file::File>),
    Set(Vec<Result<Preview, ExtractionError>>),
    StoragePersisted(bool),
}

enum State {
    Default,
    Processing,
    Complete,
}

pub struct Preview {
    _object_url: gloo_file::ObjectUrl,
    url: AttrValue,
    volume: VolumeMetadata,
}

/// UploadModal creates a modal overlay where users can upload zip archives.
/// It tries to check if the user has enabled persisted storage for the site,
///   and if not prompts the user to enable it.
pub struct UploadModal {
    previews: Vec<Result<Preview, ExtractionError>>,
    persisted: Option<bool>,
    state: State,
    cancel_click: Callback<MouseEvent>,
    cancel_drag: Callback<DragEvent>,
    onchange: Callback<Event>,
    ondrop: Callback<DragEvent>,
    prompt: Callback<MouseEvent>,
}

impl Component for UploadModal {
    type Message = Message;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let prompt = ctx.link().callback(|_| Message::Prompt);
        let cancel_click = Callback::from(|e: MouseEvent| e.stop_propagation());
        let cancel_drag = Callback::from(|e: DragEvent| e.prevent_default());
        let onchange = ctx.link().callback(|e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            Message::Process(upload_files(input.files()))
        });
        let ondrop = ctx.link().callback(|e: DragEvent| {
            e.prevent_default();
            let file_list = e.data_transfer().unwrap().files();
            Message::Process(upload_files(file_list))
        });
        Self {
            previews: vec![],
            persisted: None,
            state: State::Default,
            cancel_click,
            cancel_drag,
            onchange,
            ondrop,
            prompt,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let Props { db, .. } = ctx.props();
        match msg {
            Message::Prompt => {
                ctx.link().send_future(persist_storage());
                false
            }
            Message::Process(files) => {
                self.state = State::Processing;
                ctx.link().send_future(enclose!((db, files) process(db, files)));
                true
            }
            Message::Set(previews) => {
                self.previews = previews;
                self.state = State::Complete;
                true
            }
            Message::StoragePersisted(persisted) => {
                self.persisted = Some(persisted);
                true
            }
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_future(check());
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.persisted.is_none() { return html! {}; }
        let Props { close_modal, .. } = ctx.props();
        let (onchange, ondrop) = (&self.onchange, &self.ondrop);
        let (ondragover, ondragenter) = (&self.cancel_drag, &self.cancel_drag);
        let gallery = match self.state {
            State::Default => html! {},
            State::Processing => html! { <p>{"Processing..."}</p> },
            State::Complete => {
                let previews: Vec<Html> = self.previews.iter().map(|maybe| {
                    match maybe {
                        Ok(p) => html! {
                            <div class="preview-item">
                                <img src={&p.url} alt={&p.volume.title}/>
                                <p>{&p.volume.title}</p>
                            </div>},
                        Err(err) => html! {
                            <div class="preview-item">
                                <p>{"ERROR"}</p>
                                <p>{"failed to load file"}</p>
                                <p>{&err.filename}</p>
                            </div>
                        }
                    }
                }).collect();
                html! {<>
                    <p>{"Complete"}</p>
                    <div id="UploadPreview">
                        {previews}
                    </div>
                </>}
            }
        };
        html! {
            <div id="UploadModal" onclick={close_modal}>
                <div class="modal-content" onclick={&self.cancel_click}>
                    if let Some(false) = self.persisted {
                        <div id="storage-warning">
                            {"please persist storage to protect your files --->"}
                            <button onclick={&self.prompt}>{"persist storage"}</button>
                        </div>
                    }
                    <p class="modal-title">{ "Upload Your Mokuro Manga Files" }</p>
                    <label for="file-upload">
                        <div id="drop-container" {ondrop} {ondragover} {ondragenter}>
                            <p>{"drop Mokuro files here"}</p>
                            <p>{"or"}</p>
                            <p>{"click to browse"}</p>
                        </div>
                    </label>
                    <input id="file-upload" type="file" accept="application/zip" multiple={true} {onchange}/>
                    {gallery}
                </div>
            </div>
        }
    }
}

/// upload_modal creates a modal overlay where users can upload zip archives.
/// It tries to check if the user has enabled persisted storage for the site,
///   and if not prompts the user to enable it.

async fn check() -> Message {
    Message::StoragePersisted(is_web_storage_persisted().await.unwrap_or(true))
}

async fn persist_storage() -> Message {
    let response =
        ask_to_persist_storage().await.expect_throw("failed to persist storage");
    Message::StoragePersisted(response)
}

async fn process(db: Rc<Rexie>, files: Vec<gloo_file::File>) -> Message {
    let mut previews = Vec::with_capacity(files.len());
    for file in files.into_iter() {
        let filename = file.name();
        previews.push(
            extract_ziparchive(&db, file).await.map(|(volume, cover)| {
                let url = AttrValue::from(cover.to_string());
                Preview { _object_url: cover, url, volume }
            }).map_err(|error| ExtractionError { error, filename })
        )
    }
    Message::Set(previews)
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
