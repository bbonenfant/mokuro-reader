use enclose::enclose;
use rexie::Rexie;
use std::rc::Rc;
use wasm_bindgen::UnwrapThrowExt;
use web_sys::MouseEvent;
use yew::{html, AttrValue, Callback, Component, Context, Html, Properties};
use yew_router::components::Link;

use crate::icons;
use crate::models::{Settings, VolumeMetadata};
use crate::upload::UploadModal;
use crate::utils::db::{delete_volume, get_all_volumes_with_covers, get_settings, put_settings, put_volume};
use crate::Route;

const DELETE_PROMPT: &str = "Are you sure you want to delete this volume?\nThere is no undo!";

#[derive(Properties, PartialEq)]
pub struct Props {
    pub db: Rc<Rexie>,
}

pub enum Message {
    Noop,
    Set(Settings, Vec<GalleryItem>),
    CommitSettings(Settings),
    Delete(u32),
    UpdateVolume(u32, String),
    HideModal,
    ShowModal,
    ToggleSettingsBar,
}

/// GalleryItems are the volumes which are displayed on the home page.
pub struct GalleryItem {
    _object_url: gloo_file::ObjectUrl,
    url: AttrValue,
    volume: VolumeMetadata,
}

pub struct Home {
    modal: bool,
    sidebar: bool,
    settings: Option<Settings>,
    volumes: Vec<GalleryItem>,

    commit_settings: Callback<Settings>,
    delete_volume: Callback<u32>,
    update_volume: Callback<(u32, String)>,
    hide_modal: Callback<MouseEvent>,
    show_modal: Callback<MouseEvent>,
    toggle_settings: Callback<MouseEvent>,
}

impl Component for Home {
    type Message = Message;
    type Properties = Props;

    fn create(ctx: &Context<Self>) -> Self {
        let hide_modal = ctx.link().callback(|_| Message::HideModal);
        let show_modal = ctx.link().callback(|_| Message::ShowModal);
        let toggle_settings = ctx.link().callback(|_| Message::ToggleSettingsBar);
        let delete_volume = ctx.link().callback(|id| Message::Delete(id));
        let commit_settings = ctx.link().callback(|s| Message::CommitSettings(s));
        let update_volume = ctx.link().callback(|(id, title)| Message::UpdateVolume(id, title));
        Self {
            modal: false,
            sidebar: false,
            settings: None,
            volumes: vec![],
            commit_settings,
            delete_volume,
            update_volume,
            hide_modal,
            show_modal,
            toggle_settings,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let Props { db } = ctx.props();
        match msg {
            Message::Noop => false,
            Message::Set(settings, volumes) => {
                self.settings = Some(settings);
                self.volumes = volumes;
                true
            }
            Message::CommitSettings(settings) => {
                let old_settings = self.settings.replace(settings.clone());
                if old_settings != self.settings {
                    ctx.link().send_future(enclose!((db, settings) commit_settings(db, settings)));
                }
                false
            }
            Message::Delete(volume_id) => {
                if gloo_dialogs::confirm(&DELETE_PROMPT) {
                    ctx.link().send_future(enclose!((db) delete(db, volume_id)));
                }
                false
            }
            Message::UpdateVolume(volume_id, title) => {
                let pick = self.volumes.iter().find(|item| {
                    item.volume.id.is_some_and(|id| id == volume_id)
                });
                if let Some(item) = pick {
                    let mut volume = item.volume.clone();
                    volume.title = title.into();
                    ctx.link().send_future(enclose!((db) commit_volume(db, volume)));
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
            Message::ToggleSettingsBar => {
                self.sidebar = !self.sidebar;
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
        let (delete, update) = (&self.delete_volume, &self.update_volume);
        let gallery: Html =
            self.volumes.iter().rev().map(|v| v.render(db, delete, update)).collect();
        html! {<>
            <div id="HomeNavBar">
                <div class="nav-buttons">
                    <div class="settings" onclick={&self.toggle_settings}>{icons::gear()}{"Settings"}</div>
                    <div class="upload" onclick={&self.show_modal}>{icons::upload()}{"Upload"}</div>
                </div>
                <div class="title">{"Mokuro Library"}</div>
                <div class="nav-gh-link">
                    <a href="https://github.com/bbonenfant">{icons::github()}</a>
                </div>
            </div>
            <div id="HomeGrid">
                if let Some(data) = &self.settings {
                    <settings::SettingsBar
                        data={data.clone()}
                        expanded={self.sidebar}
                        commit={&self.commit_settings}
                    />
                }
                <div id="GalleryContainer">
                    <h2>{"Volumes"}</h2>
                    <div id="Gallery">{gallery}</div>
                </div>
            </div>
            if self.modal {
                <UploadModal {db} close_modal={&self.hide_modal}/>
            }
        </>}
    }
}

impl GalleryItem {
    fn render(
        &self,
        db: &Rc<Rexie>,
        delete_cb: &Callback<u32>,
        update_cb: &Callback<(u32, String)>,
    ) -> Html {
        let volume_id = self.volume.id.unwrap_throw();
        let onclick = delete_cb.reform(move |_| volume_id);
        let commit = update_cb.reform(move |new_title: String| (volume_id, new_title));
        let title = &self.volume.title;
        html! {
            <div class="volume-item">
                <Link<Route> to={Route::Reader {volume_id}}>
                    <img src={&self.url} alt={title}/>
                </Link<Route>>
                <title::EditableTitle {title} {commit}/>
                <download::DownloadButton {db} {volume_id}/>
                <button class="delete" {onclick}>{"Delete"}</button>
            </div>
        }
    }
}

async fn fetch(db: Rc<Rexie>) -> Message {
    let settings = get_settings(&db).await
        .expect_throw("failed to retrieve settings from IndexDB");

    let pairs = get_all_volumes_with_covers(&db).await
        .expect_throw("failed to retrieve all volumes from IndexDB");
    let mut items = Vec::with_capacity(pairs.len());
    for (volume, page) in pairs.into_iter() {
        let _object_url = gloo_file::ObjectUrl::from(page);
        let url = AttrValue::from(_object_url.to_string());
        items.push(GalleryItem { _object_url, url, volume })
    }

    Message::Set(settings, items)
}

async fn commit_settings(db: Rc<Rexie>, settings: Settings) -> Message {
    put_settings(&db, &settings).await
        .expect_throw("failed to commit settings to IndexDB");
    Message::Noop
}

async fn commit_volume(db: Rc<Rexie>, volume: VolumeMetadata) -> Message {
    put_volume(&db, &volume).await
        .expect_throw("failed to commit volume to IndexDB");
    fetch(db).await
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

mod settings {
    use web_sys::Event;
    use yew::{html, Callback, Component, Context, Html, NodeRef, Properties};

    use crate::models::{MagnifierSettings, Settings};
    use crate::utils::web::{get_input_u16, get_input_u8};

    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub data: Settings,
        pub expanded: bool,
        pub commit: Callback<Settings>,
    }

    pub struct SettingsBar {
        onchange: Callback<Event>,

        // NodeRefs
        magnifier_height_ref: NodeRef,
        magnifier_width_ref: NodeRef,
        magnifier_radius_ref: NodeRef,
        magnification_ref: NodeRef,
    }

    pub enum Message {
        Commit
    }

    impl Component for SettingsBar {
        type Message = Message;
        type Properties = Props;

        fn create(ctx: &Context<Self>) -> Self {
            let onchange = ctx.link().callback(|_| Message::Commit);
            Self {
                onchange,
                magnifier_height_ref: NodeRef::default(),
                magnifier_width_ref: NodeRef::default(),
                magnifier_radius_ref: NodeRef::default(),
                magnification_ref: NodeRef::default(),
            }
        }

        fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
            let Props { commit, data, .. } = ctx.props();
            match msg {
                Message::Commit => {
                    let magnifier_height = get_input_u16(&self.magnifier_height_ref)
                        .unwrap_or(data.magnifier.height);
                    let magnifier_width = get_input_u16(&self.magnifier_width_ref)
                        .unwrap_or(data.magnifier.width);
                    let magnifier_radius = get_input_u8(&self.magnifier_radius_ref)
                        .unwrap_or(data.magnifier.radius);
                    let magnification = get_input_u16(&self.magnification_ref)
                        .unwrap_or(data.magnifier.zoom);
                    let new_data = Settings {
                        magnifier: MagnifierSettings {
                            zoom: magnification,
                            radius: magnifier_radius,
                            height: magnifier_height,
                            width: magnifier_width,
                        }
                    };
                    if new_data != *data {
                        commit.emit(new_data);
                    }
                    false
                }
            }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            let Props { data, expanded, .. } = ctx.props();
            let hidden = !expanded;
            html! {
                <div id="SideBar" tabindex={"2"} class={"expanded"} {hidden}>
                    <hr/>
                    <h2>{"General Settings"}</h2>
                    <hr/>

                    <h3 class="sidebar-header">{"Default Magnifier Settings"}</h3>
                    <div class="sidebar-input-container">
                        <label for="height">{"Height"}</label>
                        <input
                            ref={&self.magnifier_height_ref}
                            id="height" type="number"
                            min="100" max="1000" step="10"
                            value={data.magnifier.height.to_string()}
                            onchange={&self.onchange}
                        />
                    </div>
                    <div class="sidebar-input-container">
                        <label for="width">{"Width"}</label>
                        <input
                            ref={&self.magnifier_width_ref}
                            id="width" type="number"
                            min="100" max="1000" step="10"
                            value={data.magnifier.width.to_string()}
                            onchange={&self.onchange}
                        />
                    </div>
                    <div class="sidebar-input-container">
                        <label for="radius">{"Border Radius"}</label>
                        <input
                            ref={&self.magnifier_radius_ref}
                            id="radius" type="number"
                            min="0" max="100" step="5"
                            value={data.magnifier.radius.to_string()}
                            onchange={&self.onchange}
                        />
                    </div>
                    <div class="sidebar-input-container">
                        <label for="scale">{"Magnification"}</label>
                        <input
                            ref={&self.magnification_ref}
                            id="scale" type="number"
                            min="100" max="400" step="10"
                            value={data.magnifier.zoom.to_string()}
                            onchange={&self.onchange}
                        />
                    </div>
                </div>
            }
        }
    }
}

mod title {
    use crate::utils::web::set_caret;
    use wasm_bindgen::UnwrapThrowExt;
    use web_sys::{FocusEvent, KeyboardEvent, MouseEvent};
    use yew::{html, AttrValue, Callback, Component, Context, Html, NodeRef, Properties};

    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub title: AttrValue,
        pub commit: Callback<String>,
    }

    pub struct EditableTitle {
        editing: bool,
        node_ref: NodeRef,
        onblur: Callback<FocusEvent>,
        ondblclick: Callback<MouseEvent>,
        onkeypress: Callback<KeyboardEvent>,
    }

    pub enum Message {
        BeginEdit,
        EndEdit,
    }

    impl Component for EditableTitle {
        type Message = Message;
        type Properties = Props;

        fn create(ctx: &Context<Self>) -> Self {
            let onblur = ctx.link().callback(|_| Message::EndEdit);
            let ondblclick = ctx.link().callback(|_| Message::BeginEdit);
            let onkeypress = ctx.link().batch_callback(|e: KeyboardEvent| {
                match e.code().as_str() {
                    "Enter" => { // Prevent multiline titles by catching Enter/Return.
                        e.prevent_default();
                        Some(Message::EndEdit)
                    }
                    _ => None
                }
            });
            Self { editing: false, node_ref: NodeRef::default(), onblur, ondblclick, onkeypress }
        }

        fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
            let Props { title, commit, .. } = ctx.props();
            match msg {
                Message::BeginEdit => {
                    self.editing = true;
                    set_caret(&self.node_ref);
                    true
                }
                Message::EndEdit => {
                    self.editing = false;
                    let element = self.node_ref.cast::<web_sys::HtmlElement>()
                        .expect_throw("could not resolve node reference");
                    let text = element.text_content();
                    if let Some(new_title) = text {
                        if new_title != title.as_str() {
                            commit.emit(new_title)
                        }
                    }
                    element.blur().expect_throw("failed to remove focus");
                    true
                }
            }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            let Props { title, .. } = ctx.props();
            let contenteditable = self.editing.then(|| "true");
            let onblur = &self.onblur;
            let ondblclick = &self.ondblclick;
            let onkeypress = &self.onkeypress;
            html! {
                <p ref={&self.node_ref}
                   tabindex={"1"} {contenteditable}
                   {onblur} {ondblclick} {onkeypress}
                >{title}</p>
            }
        }
    }
}