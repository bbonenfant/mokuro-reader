use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::rc::Rc;

use rexie::Rexie;
use web_sys::{Event, KeyboardEvent, MouseEvent};
use yew::{AttrValue, Callback, function_component, html, HtmlResult, NodeRef, UseStateSetter};
use yew::functional::{use_memo, use_mut_ref, use_node_ref, use_state, use_state_eq};
use yew::suspense::Suspense;
use yew_autoprops::autoprops;

use crate::models::MagnifierSettings;
use crate::utils::hooks::{use_reader_page, use_volume_reducer, VolumeAction};

#[autoprops]
#[function_component(Reader)]
pub fn reader(db: &Rc<Rexie>, volume_id: u32) -> HtmlResult {
    let volume = use_volume_reducer(db.clone(), volume_id)?;
    gloo_console::debug!("rerender: Reader");

    let cursor = use_mut_ref(|| (0i32, 0i32));
    let magnifier_style = use_state_eq(MagnifierStyle::default);
    let (left_ref, right_ref) = (use_node_ref(), use_node_ref());

    // Use a memo for the magnifier callback to prevent unnecessary renders of ReaderPage.
    let move_magnifier = use_memo(
        volume.borrow().magnifier,
        |magnifier| {
            magnifier_callback(magnifier, magnifier_style.setter(), left_ref.clone(), right_ref.clone(), cursor.clone())
        },
    );

    let onkeypress = {
        let volume = volume.clone();
        Callback::from(move |e: KeyboardEvent| {
            // gloo_console::log!("KeyCode:", e.code());
            if e.code() == "KeyZ" {
                volume.dispatch(VolumeAction::NextPage);
            }
            if e.code() == "KeyX" {
                volume.dispatch(VolumeAction::PrevPage);
            }
        })
    };

    // override the right click to toggle the magnifier
    let hide_magnifier = use_state(|| true);
    let oncontextmenu = {
        let state = hide_magnifier.clone();
        &Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            state.set(!*state);
        })
    };

    let onload = {
        let reducer = magnifier_style.clone();
        let cursor = cursor.clone();
        let (left, right) = (left_ref.clone(), right_ref.clone());
        &Callback::from(move |_: Event| {
            reducer.set(reducer.calculate(&cursor.borrow(), &left, &right))
        })
    };

    let (right, left) = {
        let v = volume.borrow();
        v.reader_state.select_pages(&v.pages)
    };

    let hidden = *hide_magnifier;
    let style = magnifier_style.to_string();
    let onmousemove = move_magnifier.as_ref();
    Ok(html! {
        <div id="Reader" tabindex="0" {oncontextmenu} {onkeypress}>
            <div class="magnifier" {hidden} {style} {onmousemove}/>
            <Suspense fallback={html!{}}>
            if let Some(page_name) = left {
                <ReaderPage {db} {volume_id} {page_name} img_ref={left_ref} {onmousemove} {onload}/>
            }
            if let Some(page_name) = right {
                <ReaderPage {db} {volume_id} {page_name} img_ref={right_ref} {onmousemove} {onload}/>
            }
            </Suspense>
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
    onload: Callback<Event>,
) -> HtmlResult {
    let (src, _ocr) = use_reader_page(db, volume_id, &page_name)?;
    gloo_console::log!("rerender", page_name.as_str());
    Ok(html! { <img ref={img_ref} class="reader-image" {src} {onmousemove} {onload}/> })
}

fn magnifier_callback(
    magnifier: &MagnifierSettings,
    setter: UseStateSetter<MagnifierStyle>,
    left_ref: NodeRef,
    right_ref: NodeRef,
    cursor: Rc<RefCell<(i32, i32)>>,
) -> Callback<MouseEvent> {
    let (zoom, m_height, m_width, radius) =
        (magnifier.zoom as i32, magnifier.height as i32, magnifier.width as i32, magnifier.radius);
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
        *cursor.borrow_mut() = (e.page_x(), e.page_y());

        let result = MagnifierStyle::calculate_with(
            &cursor.borrow(), &left_ref, &right_ref, m_height, m_width, radius, zoom,
        );

        if let Ok(value) = result {
            setter.set(value);
        }
    })
}


#[derive(Clone, Default, PartialEq)]
struct BackgroundStyle {
    url: String,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

impl Display for BackgroundStyle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { url, left, top, width, height } = self;
        write!(f, "url({url}) {left}px {top}px / {width}px {height}px no-repeat")
    }
}

#[derive(Clone, Default, PartialEq)]
struct MagnifierStyle {
    left_: Option<BackgroundStyle>,
    right_: Option<BackgroundStyle>,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    radius: u8,
    zoom: i32,
}

impl Display for MagnifierStyle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { left_, right_, left, top, width, height, radius, .. } = self;
        let background = match (left_, right_) {
            (Some(l), Some(r)) => format!("background: {l}, {r}"),
            (Some(l), None) => format!("background: {l}"),
            (None, Some(r)) => format!("background: {r}"),
            (None, None) => "".to_string(),
        };
        write!(f, "left: {left}px; top: {top}px; width: {width}px; height: {height}px; border-radius: {radius}%; {background}")
    }
}

impl MagnifierStyle {
    fn calculate(&self, cursor: &(i32, i32), left_ref: &NodeRef, right_ref: &NodeRef) -> Self {
        Self::calculate_with(cursor, left_ref, right_ref, self.height, self.width, self.radius, self.zoom).unwrap_or(self.clone())
    }

    fn calculate_with(
        cursor: &(i32, i32),
        left_ref: &NodeRef,
        right_ref: &NodeRef,
        height: i32,
        width: i32,
        radius: u8,
        zoom: i32,
    ) -> Result<Self, ()> {
        // The node refs may not resolve to a currently rendered HTML elements,
        // i.e. in the case where only one page is being displayed instead of two.
        // If neither node ref is valid, then there's no image to magnify, and so
        // we exit early.
        let left_img = left_ref.cast::<web_sys::Element>();
        let right_img = right_ref.cast::<web_sys::Element>();
        let Some(img) = left_img.as_ref().or(right_img.as_ref()) else { return Err(()) };
        let single_page = left_img.is_some() ^ right_img.is_some();

        // Get some information about the image size and position.
        let (img_height, img_width, img_top, img_left) = {
            let rect = img.get_bounding_client_rect();
            (rect.height() as i32, rect.width() as i32, rect.top() as i32, rect.left() as i32)
        };
        if img_height == 0 || img_width == 0 { return Err(()); }

        // half the height and width of the magnifier element.
        let (center_y, center_x) = (height / 2, width / 2);

        // Calculate where the center of the magnifier element should be.
        // This is not necessarily the cursor location, as we want to prevent
        // the magnifier from going outside the bounds of the background image.
        let bias = 5;
        let x = {
            let x = cursor.0 - img_left; // cursor x position
            let scale = if single_page { 1 } else { 2 }; // double the area for two pages
            x.max(bias).min((scale * img_width) - bias)
        };
        let y = {
            let y = cursor.1 - img_top; // cursor y position
            y.max(bias).min(img_height - bias)
        };

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
        let left_ = left_img.map(|element| {
            let url = element.get_attribute("src").unwrap();
            BackgroundStyle { url, left: x_shift, top: y_shift, width: z_width, height: z_height }
        });
        let right_ = right_img.map(|element| {
            let url = element.get_attribute("src").unwrap();
            let x_shift = if single_page { x_shift } else {
                center_x - (((x - img_width) * zoom) / 100)
            };
            BackgroundStyle { url, left: x_shift, top: y_shift, width: z_width, height: z_height }
        });
        // The position of the magnifier element.
        let left = img_left + x - center_x;
        let top = img_top + y - center_y;
        Ok(MagnifierStyle { left_, right_, left, top, width, height, radius, zoom })
    }
}
