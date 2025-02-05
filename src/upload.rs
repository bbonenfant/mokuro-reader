use enclose::enclose;
use rexie::Rexie;
use std::rc::Rc;
use web_sys::{DragEvent, Event, FileList, HtmlInputElement, MouseEvent};
use yew::{html, AttrValue, Callback, Component, Context, Html, Properties, TargetCast};
use yew_router::components::Link;

use crate::models::VolumeMetadata;
use crate::notify::{Notification, Notification::Warning};
use crate::utils::web::{ask_to_persist_storage, is_web_storage_persisted};
use crate::utils::zip::extract_ziparchive;
use crate::Route;

#[allow(dead_code)]
pub struct ExtractionError {
    error: crate::errors::AppError,
    filename: String,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub db: Rc<Rexie>,
    pub notify: Callback<Notification>,
    pub close_modal: Callback<MouseEvent>,
}

pub enum Message {
    Prompt,
    Process(FileList),
    Set(Vec<Result<Preview, ExtractionError>>),
    StoragePersisted(bool),
    Notify(Notification),
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
        let onchange = ctx.link().batch_callback(|e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            input.files().map(|files| Message::Process(files))
        });
        let ondrop = ctx.link().callback(|e: DragEvent| {
            e.prevent_default();
            if let Some(data) = e.data_transfer() {
                if let Some(files) = data.files() {
                    return Message::Process(files);
                }
            }
            let content = "Failed to extract data transfer - drag and drop functionality may be broken";
            Message::Notify(Warning(content, content.to_string()))
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
        let Props { db, notify, .. } = ctx.props();
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
            Message::Notify(notification) => {
                notify.emit(notification);
                false
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
                        Ok(p) => {
                            let volume_id = p.volume.id;
                            html! {
                                <Link<Route> to={Route::Reader {volume_id}}>
                                    <div class="preview-item">
                                        <img src={&p.url} alt={&p.volume.title}/>
                                        <p>{&p.volume.title}</p>
                                    </div>
                                </Link<Route>>
                            }
                        }
                        Err(err) => {
                            gloo_console::error!(err.error.to_string());
                            html! {
                                <div class="preview-item">
                                    <p>{"ERROR"}</p>
                                    <p>{"failed to load file"}</p>
                                    <p>{&err.filename}</p>
                                </div>
                            }
                        }
                    }
                }).collect();
                html! {<>
                    <p>{"Complete"}</p>
                    <hr/>
                    <div id="UploadPreview">
                        {previews}
                    </div>
                </>}
            }
        };
        html! {
            <div id="Modal" onclick={close_modal}>
                <div class="modal-content" onclick={&self.cancel_click}>
                    if let Some(false) = self.persisted {
                        <div id="storage-warning">
                            {"Please persist storage to protect your files ---> "}
                            <button onclick={&self.prompt}>{"Persist Your Storage"}</button>
                        </div>
                    }
                    <div class="close-symbol" onclick={close_modal}>{crate::icons::close()}</div>
                    <p class="modal-title">{ "Upload Your Mokuro Manga Files" }</p>
                    <p class="modal-note">
                        {"Only files generated from "}
                        <a href={"https://github.com/bbonenfant/mokuro"} target="_blank">
                            {"this Mokuro fork"}
                        </a>
                        {" are supported (*.mbz.zip)"}
                    </p>
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

async fn check() -> Message {
    Message::StoragePersisted(is_web_storage_persisted().await.unwrap_or(true))
}

async fn persist_storage() -> Message {
    match ask_to_persist_storage().await {
        Ok(response) => Message::StoragePersisted(response),
        Err(err) => Message::Notify(Warning("failed to persist storage", format!("{:?}", err)))
    }
}

async fn process(db: Rc<Rexie>, files: FileList) -> Message {
    let mut previews = Vec::with_capacity(files.length() as usize);
    for idx in 0..files.length() {
        if let Some(file) = files.item(idx) {
            let filename = file.name();
            previews.push(
                extract_ziparchive(&db, file).await.map(|(volume, cover)| {
                    let url = AttrValue::from(cover.to_string());
                    Preview { _object_url: cover, url, volume }
                }).map_err(|error| ExtractionError { error, filename })
            )
        }
    }
    Message::Set(previews)
}
