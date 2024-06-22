use std::rc::Rc;

use rexie::Rexie;
use web_sys::{KeyboardEvent, MouseEvent};
use yew::{AttrValue, Callback, function_component, html, Html, HtmlResult, use_node_ref, use_state};
use yew::suspense::{Suspense, use_future};
use yew_autoprops::autoprops;
use yew_router::{BrowserRouter, Routable, Switch};

use crate::errors::Result;
use crate::home::Home;
use crate::models::MagnifierSettings;
use crate::utils::db::{create_database, get_volume};
use crate::utils::hooks::use_reader_page;

mod utils;
mod models;
mod errors;
mod upload;
mod home;

#[function_component(App)]
fn app() -> Html {
    // We need to use Suspense in order to establish the IndexedDB connection.
    // I don't think the entrypoint component is allowed to suspend,
    //   so we need this inner component.
    #[function_component(AppWithDatabase)]
    fn inner() -> HtmlResult {
        // Create the IndexedDB client and store it within and Rc for cheap clones.
        let db = {
            let db_future = use_future(create_database)?;
            Rc::new(db_future.as_ref().expect("unable to initialize database").clone())
        };
        let render = Callback::from(move |route| switch(&db, route));

        Ok(html! {
            <BrowserRouter>
                <Switch<Route> {render} />
            </BrowserRouter>
        })
    }
    let fallback = html! {<div>{"Initialing Database..."}</div>};
    html! {<Suspense {fallback}><AppWithDatabase/></Suspense>}
}


#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    #[at("/reader/volume/:volume_id")]
    Reader { volume_id: u32 },
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(db: &Rc<Rexie>, route: Route) -> Html {
    match route {
        Route::Home => html! { <Home {db}/> },
        Route::Reader { volume_id } => html! {
            <Suspense fallback={html!{}} >
                <Reader {db} {volume_id} />
            </Suspense>
        },
        Route::NotFound => html! { <h1>{ "404" }</h1> },
    }
}

#[autoprops]
#[function_component(Reader)]
fn reader(db: &Rc<Rexie>, volume_id: u32) -> HtmlResult {
    let future = use_future(|| get_volume(db.clone(), volume_id))?;
    let volume = future.as_ref().unwrap();
    let (right, left) = (&volume.pages[0].0, &volume.pages[1].0);

    Ok(html! {
        <div id="Reader">
            <Suspense fallback={html!{}}>
                <ReaderPages {db} {volume_id} {left} {right} magnifier={volume.magnifier}/>
            </Suspense>
        </div>
    })
}

#[autoprops]
#[function_component(ReaderPages)]
fn reader_pages(
    db: &Rc<Rexie>,
    volume_id: u32,
    left: AttrValue,
    right: AttrValue,
    magnifier: &MagnifierSettings,
) -> HtmlResult {
    let (url_left, _ocr_left) = use_reader_page(db, volume_id, &left)?;
    let (url_right, _ocr_right) = use_reader_page(db, volume_id, &right)?;
    let hide_magnifier = use_state(|| true);
    let magnifier_style = use_state(String::default);
    let (left_ref, right_ref) = (use_node_ref(), use_node_ref());

    // override the right click to toggle the magnifier
    let oncontextmenu = {
        let state = hide_magnifier.clone();
        &Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            state.set(!*state);
        })
    };

    let (zoom, m_height, m_width) =
        (magnifier.zoom as i32, magnifier.height as i32, magnifier.width as i32);
    let onmousemove = {
        let (left_ref, right_ref) = (left_ref.clone(), right_ref.clone());
        let style = magnifier_style.clone();
        &Callback::from(move |e: MouseEvent| {
            // This is callback enables the "magnifier" effect around the cursor
            // when hovering over the images. This is accomplished by create a div,
            // whose center follows the location of the cursor, having a background
            // image which is a magnified version of the image being displayed and
            // positioned/shifted in such a way that the cursor is always above the
            // same location both images.
            // Adapted from here:
            //   www.w3schools.com/howto/howto_js_image_magnifier_glass.asp
            e.prevent_default();

            let left_img = left_ref.cast::<web_sys::Element>().unwrap();
            let right_img = right_ref.cast::<web_sys::Element>().unwrap();

            // Get some information about the image size and position.
            let (img_height, img_width, img_top, img_left) = {
                let rect = left_img.get_bounding_client_rect();
                (rect.height() as i32, rect.width() as i32, rect.top() as i32, rect.left() as i32,)
            };

            // half the height and width of the magnifier element.
            let (center_y, center_x) = (m_height / 2, m_width / 2);
            // Calculate where the center of the magnifier element should be.
            // This is not necessarily the cursor location, as we want to prevent
            // the magnifier from going outside the bounds of the background image.
            let bias = 5;
            let x = {
                let x = e.page_x() - img_left; // cursor x position
                x.max(bias).min((2 * img_width) - bias)
            };
            let y = {
                let y = e.page_y() - img_top; // cursor y position
                y.max(bias).min(img_height - bias)
            };

            // The size of the background image(s) (zoomed);
            let z_height = zoom * img_height / 100;
            let z_width = zoom * img_width / 100;

            // Calculate the x and y translations needed to position the
            // zoomed background image to where the magnifier is pointing.
            // This shift is relative to the magnifier element.
            // The x translation will be different for two background images.
            let padding = 0;
            let y_shift = center_y - ((y * zoom) / 100) + padding;

            // url() position / size repeat
            let bg_l = {
                let url = left_img.get_attribute("src").unwrap();
                let x_shift = center_x - ((x * zoom) / 100) + padding;
                format!("url({url}) {x_shift}px {y_shift}px / {z_width}px {z_height}px no-repeat")
            };
            let bg_r = {
                let url = right_img.get_attribute("src").unwrap();
                let x_shift = center_x - (((x - img_width) * zoom) / 100) + padding;
                format!("url({url}) {x_shift}px {y_shift}px / {z_width}px {z_height}px no-repeat")
            };

            // The position of the magnifier element.
            let left = img_left + x - center_x;
            let top = img_top + y - center_y;
            style.set(format!("left: {left}px; top: {top}px; background: {bg_l}, {bg_r}"));
        })
    };

    let hidden = *hide_magnifier;
    let style = format!(
        "width: {m_width}px; height: {m_height}px; border-radius: {}%; {}",
        magnifier.radius, *magnifier_style
    );
    Ok(html! {
        <>
        <div class="magnifier" {hidden} {style} {oncontextmenu} {onmousemove}/>
        <img ref={left_ref} class="reader-image" src={url_left} {oncontextmenu} {onmousemove} />
        <img ref={right_ref} class="reader-image" src={url_right} {oncontextmenu} {onmousemove} />
        </>
    })
}

fn main() {
    yew::Renderer::<App>::new().render();
}
