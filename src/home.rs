use enclose::enclose;
use rexie::Rexie;
use std::rc::Rc;
use wasm_bindgen::UnwrapThrowExt;
use web_sys::MouseEvent;
use yew::{html, AttrValue, Callback, Component, Context, Html, Properties};
use yew_router::components::Link;

use crate::icons;
use crate::models::{PageImage, VolumeMetadata};
use crate::upload::UploadModal;
use crate::utils::db::{delete_volume, get_all_volumes_with_covers};
use crate::Route;

const DELETE_PROMPT: &str = "Are you sure you want to delete this volume?\nThere is no undo!";

#[derive(Properties, PartialEq)]
pub struct Props {
    pub db: Rc<Rexie>,
}

pub enum Message {
    Set(Vec<(VolumeMetadata, PageImage)>),
    Delete(u32),
    HideModal,
    ShowModal,
}

/// GalleryItems are the volumes which are displayed on the home page.
struct GalleryItem {
    _object_url: gloo_file::ObjectUrl,
    url: AttrValue,
    volume: VolumeMetadata,
}

pub struct Home {
    modal: bool,
    volumes: Vec<GalleryItem>,
    delete_volume: Callback<u32>,
    hide_modal: Callback<MouseEvent>,
    show_modal: Callback<MouseEvent>,
}

impl Component for Home {
    type Message = Message;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let hide_modal = ctx.link().callback(|_| Message::HideModal);
        let show_modal = ctx.link().callback(|_| Message::ShowModal);
        let delete_volume = ctx.link().callback(|id| Message::Delete(id));
        Self {
            modal: false,
            volumes: vec![],
            delete_volume,
            hide_modal,
            show_modal,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let Props { db } = ctx.props();
        match msg {
            Message::Set(items) => {
                self.volumes = items.into_iter().map(|(volume, page)| {
                    let _object_url = gloo_file::ObjectUrl::from(page);
                    let url = AttrValue::from(_object_url.to_string());
                    GalleryItem { _object_url, url, volume }
                }).collect();
                true
            }
            Message::Delete(volume_id) => {
                if gloo_dialogs::confirm(&DELETE_PROMPT) {
                    ctx.link().send_future(enclose!((db) delete(db, volume_id)));
                }
                false
            }
            Message::HideModal => {
                self.modal = false;
                ctx.link().send_future(enclose!((db) fetch(db)));
                false
            }
            Message::ShowModal => {
                self.modal = true;
                true
            }
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let Props { db } = ctx.props();
            ctx.link().send_future(enclose!((db) fetch(db)))
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let Props { db } = ctx.props();
        let onclick = &self.show_modal;
        let delete = &self.delete_volume;
        let gallery: Html =
            self.volumes.iter().rev().map(|v| v.render(db, delete)).collect();
        html! {<>
            <div id="HomeTopBar">
                <div class="nav-buttons">
                    <div class="settings">{icons::gear()}{"Settings"}</div>
                    <div class="upload" onclick={onclick}>{icons::upload()}{"Upload"}</div>
                </div>
                <div class="title">{"Mokuro Library"}</div>
                <div class="nav-gh-link">
                    <a href="https://github.com/bbonenfant">{icons::github()}</a>
                </div>
            </div>
            <h2>{"Volumes"}</h2>
            <div id="Gallery">{gallery}</div>
            if self.modal {
                <UploadModal {db} close_modal={&self.hide_modal}/>
            }
        </>}
    }
}

impl GalleryItem {
    fn render(&self, db: &Rc<Rexie>, delete_cb: &Callback<u32>) -> Html {
        let volume_id = self.volume.id.unwrap_throw();
        let onclick = delete_cb.reform(move |_| volume_id);
        html! {
            <div class="volume-item">
                <Link<Route> to={Route::Reader {volume_id}}>
                    <img src={&self.url} alt={&self.volume.title}/>
                </Link<Route>>
                <p>{&self.volume.title}</p>
                <download::DownloadButton {db} {volume_id}/>
                <button class="delete" {onclick}>{"Delete"}</button>
            </div>
        }
    }
}

async fn fetch(db: Rc<Rexie>) -> Message {
    let items = get_all_volumes_with_covers(&db).await
        .expect_throw("failed to retrieve all volumes from IndexDB");
    Message::Set(items)
}

async fn delete(db: Rc<Rexie>, volume_id: u32) -> Message {
    delete_volume(&db, volume_id).await
        .expect_throw("failed to delete volume from IndexDB");
    fetch(db).await
}

mod download {
    use enclose::enclose;
    use rexie::Rexie;
    use std::cmp::PartialEq;
    use std::rc::Rc;
    use wasm_bindgen::UnwrapThrowExt;
    use web_sys::MouseEvent;
    use yew::{html, AttrValue, Callback, Component, Context, Html, Properties};

    use crate::utils::zip::create_ziparchive;

    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub db: Rc<Rexie>,
        pub volume_id: u32,
    }

    pub enum Message {
        Request,
        Set(gloo_file::File),
    }


    enum State {
        Default,
        Processing,
        Ready(File),
    }

    struct File {
        _url_object: gloo_file::ObjectUrl,
        file: gloo_file::File,
        url: AttrValue,
    }

    pub struct DownloadButton {
        state: State,
        onclick: Callback<MouseEvent>,
    }

    impl Component for DownloadButton {
        type Message = Message;
        type Properties = Props;

        fn create(ctx: &Context<Self>) -> Self {
            let onclick = ctx.link().callback(|_| Message::Request);
            Self {
                state: State::Default,
                onclick,
            }
        }

        fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
            let Props { db, volume_id } = ctx.props();
            match msg {
                Message::Request => {
                    self.state = State::Processing;
                    ctx.link().send_future(enclose!(
                        (db, volume_id) fetch(db, volume_id)
                    ));
                    true
                }
                Message::Set(file) => {
                    let _url_object = gloo_file::ObjectUrl::from(file.clone());
                    let url = AttrValue::from(_url_object.to_string());
                    self.state = State::Ready(File { _url_object, file, url });
                    true
                }
            }
        }

        fn view(&self, _ctx: &Context<Self>) -> Html {
            let class = "download";
            match &self.state {
                State::Default => {
                    html! { <button {class} onclick={&self.onclick}>{"Prepare Download"}</button> }
                }
                State::Processing => html! { <button {class}>{"Preparing..."}</button> },
                State::Ready(file) => {
                    html! {
                        <a href={&file.url} download={file.file.name()}>
                            <button {class}>{"Download"}</button>
                        </a>
                    }
                }
            }
        }
    }

    async fn fetch(db: Rc<Rexie>, volume_id: u32) -> Message {
        let file = create_ziparchive(db.clone(), volume_id).await
            .expect_throw("failed to create zip archive for download");
        Message::Set(file)
    }
}
