use std::fmt::{Display, Formatter};
use std::rc::Rc;

use enclose::enclose;
use rexie::Rexie;
use wasm_bindgen::{JsCast, UnwrapThrowExt};
use web_sys::{ClipboardEvent, Event, FocusEvent, KeyboardEvent, MouseEvent};
use yew::{AttrValue, Callback, function_component, html, Html, HtmlResult, NodeRef};
use yew::functional::{use_effect_with, use_memo, use_mut_ref, use_node_ref, use_state};
use yew::suspense::Suspense;
use yew_autoprops::autoprops;
use yew_hooks::{use_event_with_window, use_toggle};

use crate::models::{MagnifierSettings, OcrBlock};
use crate::utils::hooks::{CursorAction, PageAction, use_cursor, use_page_reducer, use_volume_reducer, VolumeAction};
use crate::utils::web::{get_screen_size, get_selected_text};

#[autoprops]
#[function_component(Reader)]
pub fn reader(db: &Rc<Rexie>, volume_id: u32) -> HtmlResult {
    let volume = use_volume_reducer(db.clone(), volume_id)?;
    gloo_console::debug!("rerender: Reader");

    // State
    let (cursor, c_signal) = use_cursor();
    let editable = use_toggle(false, true);
    let (reader, left, right) = (use_node_ref(), use_node_ref(), use_node_ref());
    let window = use_state(WindowState::default);

    let (right_page, left_page) =
        volume.data.reader_state.select_pages(&volume.data.pages);

    // Focus on the reader div when pages change.
    // This is so that the keyboard shortcuts will be caught and handled
    //  without needing to click the page.
    { // For some reason, using enclose! here causes use_effect to not fire.
        let ref_ = reader.clone();
        use_effect_with((right_page.clone(), left_page.clone()), move |_| {
            gloo_console::log!("focus");
            let _ = ref_.cast::<web_sys::HtmlElement>().unwrap().focus();
        })
    }

    // These are the keyboard shortcuts/commands.
    let handle_keypress = use_memo((), enclose!((editable, volume) |_| {
        Callback::from(move |e: KeyboardEvent| {
            // gloo_console::log!("KeyCode:", e.code());
            if e.code() == "KeyE" { editable.toggle(); }
            else if e.code() == "KeyX" { volume.dispatch(VolumeAction::PrevPage); }
            else if e.code() == "KeyZ" { volume.dispatch(VolumeAction::NextPage); }
        })
    }));

    // Hook into resize event of the window.
    // We need to keep track of the size of the page images.
    use_event_with_window("resize", enclose!((left, right, window) move |_: Event| {
        let screen = Screen::new();
        let left = Rect::try_from(&left).unwrap_or(window.left);
        let right = Rect::try_from(&right).unwrap_or(window.right);
        window.set(WindowState { screen, left, right })
    }));

    // Track cursor movements. This needed for the magnifier.
    let update_cursor = use_memo((), enclose!((c_signal) |_| {
        Callback::from(move |e| { c_signal.dispatch(CursorAction::Update(e)) })
    }));

    // This callback is intended for when the images finish loading.
    // This registers the size of the page images and then force re-renders
    // the magnifier component. The force re-render is necessary to update
    // the background images, allowing the effect to seamlessly work across pages.
    let on_image_load = use_memo((), enclose!((c_signal, left, right, window) |_|
        Callback::from(move |_: Event| {
            c_signal.dispatch(CursorAction::ForceRerender);
            let left = Rect::try_from(&left).unwrap_or(window.left);
            let right = Rect::try_from(&right).unwrap_or(window.right);
            window.set(WindowState { screen: window.screen, left, right });
        })
    ));

    // Override the right click to toggle the magnifier
    let handle_right_click = use_memo((), enclose!((c_signal) move |_|
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            c_signal.dispatch(CursorAction::Toggle);
        })
    ));

    let show_magnifier = cursor.magnify;
    let editable = *editable;
    let settings = volume.data.magnifier;
    let (cursor, force) = (cursor.position, cursor.force);
    let (left, right) = (&left, &right);
    let oncontextmenu = handle_right_click.as_ref();
    let onkeypress = handle_keypress.as_ref();
    let onload = on_image_load.as_ref();
    let onmousemove = update_cursor.as_ref();
    Ok(html! {
        <div ref={reader} id="Reader" tabindex="0" {oncontextmenu} {onkeypress} {onmousemove}>
            if show_magnifier { <Magnifier {cursor} {settings} {left} {right} {force}/> }
            <Suspense fallback={html!{}}>
            if let Some(name) = left_page {
                <ReaderPage {db} {volume_id} {name} img_ref={left} rect={window.left} {editable} {onload}/>
            }
            if let Some(name) = right_page {
                <ReaderPage {db} {volume_id} {name} img_ref={right} rect={window.right} {editable} {onload}/>
            }
            </Suspense>
        </div>
    })
}

#[autoprops]
#[function_component(Magnifier)]
fn magnifier(
    cursor: (i32, i32),
    settings: &MagnifierSettings,
    left: &NodeRef,
    right: &NodeRef,
    force: u64,
) -> Html {
    let _ = force;
    let style = use_mut_ref(MagnifierStyle::default);
    let result = MagnifierStyle::compute(&cursor, left, right, settings);
    if let Ok(value) = result {
        *style.borrow_mut() = value;
    }
    let style = style.borrow().to_string();
    html! {<div class="magnifier" {style}/>}
}

#[autoprops]
#[function_component(ReaderPage)]
fn reader_page(
    db: &Rc<Rexie>,
    volume_id: u32,
    name: AttrValue,
    img_ref: &NodeRef,
    rect: &Rect,
    editable: bool,
    onload: Callback<Event>,
) -> HtmlResult {
    let reducer = use_page_reducer(db.clone(), volume_id, name)?;
    let signal = reducer.dispatcher();

    let update_db = Callback::from(enclose!((signal)
        move |block: OcrBlock| signal.dispatch(PageAction::UpdateBlock(block))
    ));
    // gloo_console::log!("rerender", page_name.as_str());

    let rect = *rect;
    let src = &reducer.url;
    let scale = rect.scale(reducer.ocr.img_height);
    let Rect { top, left, height, width, .. } = rect;
    let style = format!(
        "position: absolute; \
         top: {top:.2}px; left: {left:.2}px; \
         height: {height:.2}px; width: {width:.2}px; \
         border: 2px solid red;",
    );
    Ok(html! {
        <>
        <div {style}  />
        <img ref={img_ref} class="reader-image" {src} {onload}/>
        {
            reducer.ocr.blocks.iter().map(|block| {
                html!{<OcrTextBox {editable} {rect} {scale} block={block.clone()} update_db={&update_db}/>}
            }).collect::<Html>()
        }
        </>
    })
}

#[autoprops]
#[function_component(OcrTextBox)]
fn ocr_text_block(
    editable: bool,
    rect: &Rect,
    scale: f64,
    block: &OcrBlock,
    update_db: &Callback<OcrBlock>,
) -> Html {
    let top = rect.top + ((block.box_.1 as f64) / scale);
    let left = rect.left + ((block.box_.0 as f64) / scale);
    let height = ((block.box_.3 - block.box_.1) as f64) / scale;
    let width = ((block.box_.2 - block.box_.0) as f64) / scale;
    let mode = if block.vertical { "vertical-rl" } else { "horizontal-tb" };
    let font = (block.font_size as f64) / scale;
    let style = format!(
        "top: {top:.2}px; left: {left:.2}px; \
         min-height: {height:.2}px; min-width: {width:.2}px; \
         font-size: {font:.1}px; writing-mode: {mode};"
    );

    let onblur = enclose!((block) update_db.reform(move |e: FocusEvent| {
        gloo_console::log!("onblur");
        let target = e.target().unwrap();
        let div = target.dyn_ref::<web_sys::HtmlElement>().unwrap();
        let children = div.children();
        let mut lines = vec![];
        for index in 0..children.length() {
            let child = children.item(index).unwrap();
            if let Some(text) = child.text_content() {
                lines.push(AttrValue::from(text))
            }
        }
        OcrBlock { lines, uuid: block.uuid.clone(), ..block }
    }));

    let remove_newlines = use_memo((), |_|
    Callback::from(move |e: Event| {
        let e = e.dyn_into::<ClipboardEvent>()
            .expect_throw("couldn't convert to ClipboardEvent");
        if let Some(text) = get_selected_text() {
            if let Some(clipboard) = e.clipboard_data() {
                clipboard.set_data("text/plain", &text.replace('\n', ""))
                    .expect("couldn't write to clipboard");
                e.prevent_default();
            }
        }
    }),
    );

    let key = block.uuid.as_str();
    let contenteditable = if editable { Some("true") } else { None };
    let draggable = Some("false");
    let oncopy = remove_newlines.as_ref();
    let onfocus = Callback::from(|_| gloo_console::log!("onfocus"));
    let onkeypress = Callback::from(|e: KeyboardEvent| e.set_cancel_bubble(true));
    html! {
        <div {key} class="ocr-block" {contenteditable} {draggable} {style} {onblur} {oncopy} {onfocus} {onkeypress}>
            {block.lines.iter().map(|line| html!{<p>{line}</p>}).collect::<Html>()}
        </div>
    }
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
    fn compute(
        cursor: &(i32, i32),
        left_ref: &NodeRef,
        right_ref: &NodeRef,
        magnifier: &MagnifierSettings,
    ) -> Result<Self, ()> {
        let (zoom, height, width, radius) =
            (magnifier.zoom as i32, magnifier.height as i32, magnifier.width as i32, magnifier.radius);

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

#[derive(Copy, Clone, PartialEq)]
struct Screen {
    width: u32,
    height: u32,
}

impl Screen {
    fn new() -> Self {
        let (width, height) = get_screen_size();
        Self { width, height }
    }
}

impl Default for Screen {
    fn default() -> Self {
        let (width, height) = get_screen_size();
        Self { width, height }
    }
}

impl Display for Screen {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Screen({}px, {}px)", self.width, self.height)
    }
}

#[derive(Copy, Clone, Default, PartialEq)]
struct Rect {
    top: f64,
    left: f64,
    bottom: f64,
    right: f64,
    height: f64,
    width: f64,
}

impl Rect {
    fn scale(&self, height: u32) -> f64 {
        (height as f64) / self.height
    }
}

impl From<web_sys::DomRect> for Rect {
    fn from(value: web_sys::DomRect) -> Self {
        Self {
            top: value.top(),
            left: value.left(),
            bottom: value.bottom(),
            right: value.right(),
            height: value.height(),
            width: value.width(),
        }
    }
}

impl TryFrom<&NodeRef> for Rect {
    type Error = ();
    fn try_from(value: &NodeRef) -> Result<Self, Self::Error> {
        value.cast::<web_sys::Element>().map(|element| {
            element.get_bounding_client_rect().into()
        }).ok_or(())
    }
}

#[derive(Copy, Clone, Default)]
struct WindowState {
    screen: Screen,
    left: Rect,
    right: Rect,
}
