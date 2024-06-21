use std::rc::Rc;

use rexie::Rexie;
use web_sys::MouseEvent;
use yew::{AttrValue, Callback, function_component, html, Html, HtmlResult, use_mut_ref, use_state};
use yew::suspense::{Suspense, use_future, use_future_with};
use yew_autoprops::autoprops;

use crate::errors::Result;
use crate::upload::UploadModal;
use crate::utils::db::{create_database, get_all_volumes, get_page};
use crate::utils::timestamp;

mod utils;
mod models;
mod errors;
mod upload;


#[function_component(App)]
fn app() -> HtmlResult {
    // Create the IndexedDB client and store it within and Rc for cheap clones.
    let db = {
        let db_future = use_future(create_database)?;
        Rc::new(db_future.as_ref().expect("unable to initialize database").clone())
    };
    let rerender = use_state(timestamp);

    // state and callbacks for showing the file upload modal
    let should_show_upload_modal = use_state(|| false);
    let show_upload_modal: Callback<MouseEvent> = {
        let should_show_upload_modal = should_show_upload_modal.clone();
        Callback::from(move |_| { should_show_upload_modal.set(true) })
    };
    let hide_upload_modal: Callback<MouseEvent> = {
        let rerender = rerender.clone();
        let should_show_upload_modal = should_show_upload_modal.clone();
        Callback::from(move |_| {
            rerender.set(timestamp());  // trigger rerender of gallery
            should_show_upload_modal.set(false)
        })
    };

    Ok(html! {
        <div>
            <button onclick={show_upload_modal}>{"Upload"}</button>
            <h1>{"Mokuro App"}</h1>
            <h2>{"Hello World"}</h2>
            <Gallery db={db.clone()} rerender={*rerender}/>
            if *should_show_upload_modal {
                <Suspense fallback={Html::default()}>
                    <UploadModal {db} close_modal={hide_upload_modal} rerender={*rerender}/>
                </Suspense>
            }
        </div>
    })
}

#[autoprops]
#[function_component(Gallery)]
fn gallery(db: &Rc<Rexie>, rerender: u64) -> HtmlResult {
    let future = use_future_with(rerender, |_| get_all_volumes(db.clone()))?;
    let volumes = future.as_ref().expect("failed to get all volumes");
    Ok(html! {
        <div class="flexbox">{
            volumes.iter().map(|volume| {
                let cover = volume.cover();
                let title = &volume.title;
                let volume_id = volume.id.unwrap();
                html!{<GalleryVolume key={volume_id} {db} {volume_id} {title} {cover}/>}
            }).collect::<Html>()
        }</div>
    })
}

#[autoprops]
#[function_component(GalleryVolume)]
fn gallery_volume(db: &Rc<Rexie>, volume_id: u32, title: AttrValue, cover: AttrValue) -> HtmlResult {
    let state = use_mut_ref(|| None);  // use_state causes rerender
    let page = {
        let future = use_future_with(
            cover.clone(), |_| get_page(db.clone(), volume_id, cover.clone()),
        )?;
        future.as_ref().unwrap().clone()
    };

    let object_url = gloo_file::ObjectUrl::from(page);
    let src = object_url.to_string();
    // Store the ObjectUrl in state, as on drop the URL is revoked.
    *state.borrow_mut() = Some(object_url);

    Ok(html! {
        <div id="VolumeCover">
            <img {src} alt={&title}/>
            <p>{title}</p>
        </div>
    })
}

fn main() {
    yew::Renderer::<_App>::new().render();
}

#[function_component(_App)]
fn _app() -> Html {
    html! {
        <Suspense fallback={html! {<div>{"Initialing Database..."}</div>}}>
            <App/>
        </Suspense>
    }
}