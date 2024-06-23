use std::rc::Rc;

use rexie::Rexie;
use web_sys::MouseEvent;
use yew::{AttrValue, Callback, function_component, html, HtmlResult, NodeRef};
use yew::functional::{use_node_ref, use_state, UseStateSetter};
use yew::suspense::{Suspense, use_future};
use yew_autoprops::autoprops;

use crate::models::MagnifierSettings;
use crate::utils::db::get_volume;
use crate::utils::hooks::use_reader_page;

#[autoprops]
#[function_component(Reader)]
pub fn reader(db: &Rc<Rexie>, volume_id: u32) -> HtmlResult {
    let future = use_future(|| get_volume(db.clone(), volume_id))?;
    let volume = future.as_ref().unwrap();

    let magnifier_style = use_state(String::default);
    let (left_ref, right_ref) = (use_node_ref(), use_node_ref());
    let onmousemove =
        &magnifier_callback(&volume.magnifier, magnifier_style.setter(), left_ref.clone(), right_ref.clone());

    // override the right click to toggle the magnifier
    let hide_magnifier = use_state(|| true);
    let oncontextmenu = {
        let state = hide_magnifier.clone();
        &Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            state.set(!*state);
        })
    };

    let (right, left) = volume.reader_state.select_pages(&volume.pages);
    let hidden = *hide_magnifier;
    let style = {
        let MagnifierSettings { width, height, radius, .. } = &volume.magnifier;
        format!("width: {width}px; height: {height}px; border-radius: {radius}%; {}", *magnifier_style)
    };
    Ok(html! {
        <div id="Reader" {oncontextmenu}>
            <div class="magnifier" {hidden} {style} {onmousemove}/>
            if let Some(page_name) = left {
                <Suspense fallback={html!{}}>
                    <ReaderPage {db} {volume_id} {page_name} img_ref={left_ref} {onmousemove} />
                </Suspense>
            }
            if let Some(page_name) = right {
                <Suspense fallback={html!{}}>
                    <ReaderPage {db} {volume_id} {page_name} img_ref={right_ref} {onmousemove} />
                </Suspense>
            }
        </div>
    })
}

#[autoprops]
#[function_component(ReaderPage)]
fn reader_page(
    db: &Rc<Rexie>,
    volume_id: u32,
    page_name: AttrValue,
    img_ref: &NodeRef,
    onmousemove: &Callback<MouseEvent>,
) -> HtmlResult {
    let (src, _ocr) = use_reader_page(db, volume_id, &page_name)?;
    Ok(html! { <img ref={img_ref} class="reader-image" {src} {onmousemove} /> })
}

fn magnifier_callback(
    magnifier: &MagnifierSettings,
    setter: UseStateSetter<String>,
    left_ref: NodeRef,
    right_ref: NodeRef,
) -> Callback<MouseEvent> {
    let (zoom, m_height, m_width) =
        (magnifier.zoom as i32, magnifier.height as i32, magnifier.width as i32);
    Callback::from(move |e: MouseEvent| {
        // This is callback enables the "magnifier" effect around the cursor
        // when hovering over the images. This is accomplished by create a div,
        // whose center follows the location of the cursor, having a background
        // image which is a magnified version of the image being displayed and
        // positioned/shifted in such a way that the cursor is always above the
        // same location both images.
        // Adapted from here:
        //   www.w3schools.com/howto/howto_js_image_magnifier_glass.asp
        e.prevent_default();

        // The node refs may not resolve to a currently rendered HTML elements,
        // i.e. in the case where only one page is being displayed instead of two.
        // If neither node ref is valid, then there's no image to magnify, and so
        // we exit early.
        let left_img = left_ref.cast::<web_sys::Element>();
        let right_img = right_ref.cast::<web_sys::Element>();
        let Some(img) = left_img.as_ref().or(right_img.as_ref()) else { return };
        let single_page = left_img.is_some() ^ right_img.is_some();

        // Get some information about the image size and position.
        let (img_height, img_width, img_top, img_left) = {
            let rect = img.get_bounding_client_rect();
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
            let scale = if single_page { 1 } else { 2 }; // double the area for two pages
            x.max(bias).min((scale * img_width) - bias)
        };
        let y = {
            let y = e.page_y() - img_top; // cursor y position
            y.max(bias).min(img_height - bias)
        };

        // construct the value of the background CSS property.
        let background = {
            // The size of the background image(s) (zoomed);
            let z_height = zoom * img_height / 100;
            let z_width = zoom * img_width / 100;

            // Calculate the x and y translations needed to position the
            // zoomed background image to where the magnifier is pointing.
            // This shift is relative to the magnifier element.
            // The x translation will be different for two background images.
            let x_shift = center_x - ((x * zoom) / 100);
            let y_shift = center_y - ((y * zoom) / 100);

            // css format: url() position / size repeat
            let background_left = left_img.map(|element| {
                let url = element.get_attribute("src").unwrap();
                format!("url({url}) {x_shift}px {y_shift}px / {z_width}px {z_height}px no-repeat")
            });
            let background_right = right_img.map(|element| {
                let url = element.get_attribute("src").unwrap();
                let x_shift = if single_page { x_shift } else {
                    center_x - (((x - img_width) * zoom) / 100)
                };
                format!("url({url}) {x_shift}px {y_shift}px / {z_width}px {z_height}px no-repeat")
            });
            match (background_left, background_right) {
                (Some(l), Some(r)) => [l, r].join(", "),
                (Some(l), None) => l,
                (None, Some(r)) => r,
                _ => unreachable!()
            }
        };

        // The position of the magnifier element.
        let left = img_left + x - center_x;
        let top = img_top + y - center_y;
        setter.set(format!("left: {left}px; top: {top}px; background: {background}"));
    })
}
