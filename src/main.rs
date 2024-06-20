use std::rc::Rc;

use web_sys::MouseEvent;
use yew::{Callback, function_component, html, Html, HtmlResult, use_state};
use yew::suspense::{Suspense, use_future};

use crate::errors::Result;
use crate::upload::UploadModal;
use crate::utils::db::create_database;
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

    // state and callbacks for showing the file upload modal
    let should_show_upload_modal = use_state(|| false);
    let show_upload_modal: Callback<MouseEvent> = {
        let should_show_upload_modal = should_show_upload_modal.clone();
        Callback::from(move |_| { should_show_upload_modal.set(true) })
    };
    let hide_upload_modal: Callback<MouseEvent> = {
        let should_show_upload_modal = should_show_upload_modal.clone();
        Callback::from(move |_| { should_show_upload_modal.set(false) })
    };

    Ok(html! {
        <div>
            <button onclick={show_upload_modal}>{"Upload"}</button>
            <h1>{"Mokuro App"}</h1>
            <h2>{"Hello World"}</h2>
            if *should_show_upload_modal {
                <Suspense fallback={html! {<></>}}>
                    <UploadModal {db} close_modal={hide_upload_modal} timestamp={timestamp()}/>
                </Suspense>
            }
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