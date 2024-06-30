use std::fmt::{Display, Formatter};
use std::rc::Rc;

use enclose::enclose;
use rexie::Rexie;
use wasm_bindgen::{JsCast, UnwrapThrowExt};
use web_sys::{Event, KeyboardEvent, MouseEvent};
use yew::{Callback, classes, function_component, html, Html, HtmlResult, NodeRef};
use yew::functional::{use_effect, use_memo, use_mut_ref, use_node_ref, use_state_eq};
use yew_autoprops::autoprops;
use yew_hooks::{use_event_with_window, use_toggle};

use crate::models::MagnifierSettings;
pub use crate::reader::window::{BoundingBox, Rect, WindowState};
use crate::utils::hooks::{
    cursor::{CursorAction, use_cursor}

    ,
    volume::{use_volume_reducer, VolumeAction},
};

#[autoprops]
#[function_component(Reader)]
pub fn reader(db: &Rc<Rexie>, volume_id: u32) -> HtmlResult {
    let volume = use_volume_reducer(db.clone(), volume_id)?;
    gloo_console::debug!("rerender: Reader");

    // State
    let (cursor, c_signal) = use_cursor();
    let editable = use_toggle(false, true);
    let (reader, left, right) = (use_node_ref(), use_node_ref(), use_node_ref());
    let window = use_state_eq(WindowState::default);

    let (right_page, left_page) =
        volume.data.reader_state.select_pages(&volume.data.pages);

    // Focus on the reader div when pages change.
    // This is so that the keyboard shortcuts will be caught and handled
    //  without needing to click the page.
    { // For some reason, using enclose! here causes use_effect to not fire.
        let ref_ = reader.clone();
        use_effect(move || {
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
        let left = Rect::try_from(&left).unwrap_or(window.left.rect);
        let right = Rect::try_from(&right).unwrap_or(window.right.rect);
        window.set(WindowState::new(left, right))
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
            let left = Rect::try_from(&left).unwrap_or(window.left.rect);
            let right = Rect::try_from(&right).unwrap_or(window.right.rect);
            window.set(WindowState::new(left, right))
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

    let class = classes!(editable.then(||Some("editable")));
    Ok(html! {
        <div ref={reader} id="Reader" {class} tabindex="0" {oncontextmenu} {onkeypress} {onmousemove}>
            if show_magnifier { <Magnifier {cursor} {settings} {left} {right} {force}/> }

            if let Some(name) = left_page {
                <page::Page {db} {volume_id} {name} node_ref={left} bbox={window.left} mutable={editable} {onload} />
            }
            if let Some(name) = right_page {
                <page::Page {db} {volume_id} {name} node_ref={right} bbox={window.right} mutable={editable} {onload} />
            }

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
    html! {<div id="Magnifier" class="magnifier" {style}/>}
}

mod page {
    use std::rc::Rc;

    use enclose::enclose;
    use rexie::Rexie;
    use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
    use web_sys::{ClipboardEvent, KeyboardEvent};
    use yew::{AttrValue, Callback, Component, Context, Event, Html, html, NodeRef, Properties};

    use crate::errors::AppError;
    use crate::models::{OcrBlock, PageImage, PageOcr};
    use crate::reader::BoundingBox;
    use crate::utils::db::{get_page_and_ocr, put_ocr};
    use crate::utils::web::get_selection;

    const DELETE_PROMPT: &str = "Are you sure you want to delete this?\nThere is no undo!";

    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub db: Rc<Rexie>,
        pub volume_id: u32,
        pub name: AttrValue,
        pub node_ref: NodeRef,
        pub bbox: BoundingBox,
        pub mutable: bool,
        pub onload: Callback<Event>,
    }

    pub enum PageMessage {
        Set(PageImage, PageOcr),
        Refresh(bool),
        DeleteBlock(AttrValue),
        UpdateBlock(OcrBlock),
    }

    pub struct Page {
        _url_object: Option<gloo_file::ObjectUrl>,
        ocr: PageOcr,
        url: AttrValue,

        commit: Callback<Option<OcrBlock>>,
        delete: Callback<Option<AttrValue>>,
        oncopy: Callback<Event>,
    }

    impl Component for Page {
        type Properties = Props;
        type Message = PageMessage;
        fn create(ctx: &Context<Self>) -> Self {
            let commit = ctx.link().batch_callback(
                |block: Option<OcrBlock>|
                block.map(|b| Self::Message::UpdateBlock(b))
            );
            let delete = ctx.link().batch_callback(
                |uuid: Option<AttrValue>|
                uuid.map(|uuid| Self::Message::DeleteBlock(uuid))
            );
            let oncopy = Callback::from(|e: Event| {
                let e = e.dyn_into::<ClipboardEvent>()
                    .expect_throw("couldn't convert to ClipboardEvent");
                let selection = get_selection().and_then(|s| s.to_string().as_string());
                if let Some(text) = selection {
                    if let Some(clipboard) = e.clipboard_data() {
                        clipboard.set_data("text/plain", &text.replace('\n', ""))
                            .expect("couldn't write to clipboard");
                        e.prevent_default();
                    }
                }
            });
            Self {
                _url_object: None,
                ocr: PageOcr::default(),
                url: AttrValue::default(),
                commit,
                delete,
                oncopy,
            }
        }

        fn changed(&mut self, ctx: &Context<Self>, previous: &Self::Properties) -> bool {
            let Props { db, volume_id, name, .. } = ctx.props();
            if *volume_id != previous.volume_id || name != &previous.name {
                self._url_object = None;  // TODO: reconsider
                ctx.link().send_future(enclose!(
                    (db, volume_id => id, name) Self::fetch(db, id, name)
                ));
                return false;
            }
            return true;
        }

        fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
            match msg {
                PageMessage::Set(image, ocr) => {
                    let object = gloo_file::ObjectUrl::from(image);
                    self.url = AttrValue::from(object.to_string());
                    self._url_object = Some(object);
                    self.ocr = ocr;
                    true
                }
                PageMessage::Refresh(_) => {
                    false
                }
                PageMessage::DeleteBlock(uuid) => {
                    let index = self.ocr.blocks.iter()
                        .position(|b| b.uuid == uuid).unwrap();
                    self.ocr.blocks.remove(index);

                    let ocr = self.ocr.clone();
                    let Props { db, volume_id, name, .. } = ctx.props();
                    ctx.link().send_future(enclose!(
                        (db, volume_id => id, name) Self::commit_ocr(db, id, name, ocr)
                    ));
                    true
                }
                PageMessage::UpdateBlock(block) => {
                    let index = self.ocr.blocks.iter()
                        .position(|b| b.uuid == block.uuid).unwrap();
                    self.ocr.blocks[index] = block;

                    let ocr = self.ocr.clone();
                    let Props { db, volume_id, name, .. } = ctx.props();
                    ctx.link().send_future(enclose!(
                        (db, volume_id => id, name) Self::commit_ocr(db, id, name, ocr)
                    ));
                    true
                }
            }
        }

        fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
            if first_render {
                let Props { db, volume_id, name, .. } = ctx.props();
                ctx.link().send_future(enclose!(
                    (db, volume_id => id, name) Self::fetch(db, id, name)
                ))
            }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            if self._url_object.is_none() {
                return Html::default();
            }

            let Props { bbox, node_ref, onload, mutable, .. } = ctx.props();
            let bbox = *bbox;
            let draggable = Some("false");
            let oncopy = &self.oncopy;
            let src = &self.url;
            let scale = (self.ocr.img_height as f64) / bbox.rect.height;
            html! {
                <>
                <img ref={node_ref} class="reader-image" {draggable} {src} {onload}/>
                {
                    self.ocr.blocks.iter().map(|block| {
                        let backspace_delete = {
                            let uuid = block.uuid.clone();
                            self.delete.reform(
                                move |e: KeyboardEvent| {
                                    if e.code() == "Backspace" {
                                        if gloo_dialogs::confirm(DELETE_PROMPT) {
                                            return Some(uuid.clone())
                                        }
                                    }
                                    None
                                }
                            )
                        };
                        let key = block.uuid.as_str();
                        let block = block.clone();
                        let commit_block = &self.commit;
                        html!{<super::ocr::TextBlock {key} {mutable} {bbox} {scale} {block} {commit_block} {backspace_delete} {oncopy}/>}
                    }).collect::<Html>()
                }
                </>
            }
        }
    }

    impl Page {
        async fn fetch(db: Rc<Rexie>, id: u32, name: AttrValue) -> PageMessage {
            let key = js_sys::Array::of2(&id.into(), &name.as_str().into());
            let (image, ocr) = get_page_and_ocr(&db, &key.into()).await
                .expect_throw("failed to get Page and Ocr data from IndexedDB");
            PageMessage::Set(image, ocr)
        }
        async fn commit_ocr(db: Rc<Rexie>, id: u32, name: AttrValue, ocr: PageOcr) -> PageMessage {
            let key = js_sys::Array::of2(&id.into(), &name.as_str().into());
            put_ocr(&db, &ocr, &key).await.unwrap_or_else(|error| {
                if let AppError::RexieError(err) = error {
                    gloo_console::error!(JsValue::from(err));
                }
            });
            PageMessage::Refresh(true)
        }
    }
}

mod ocr {
    use wasm_bindgen::{JsCast, UnwrapThrowExt};
    use web_sys::{Event, FocusEvent, KeyboardEvent, MouseEvent};
    use yew::{AttrValue, Callback, Component, Context, Html, html, NodeRef, Properties};
    use yew::html::Scope;

    use drag::Drag;

    use crate::models::OcrBlock;
    use crate::reader::BoundingBox;

    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub bbox: BoundingBox,
        pub block: OcrBlock,
        pub mutable: bool,
        pub scale: f64,

        pub backspace_delete: Callback<KeyboardEvent>,
        pub commit_block: Callback<Option<OcrBlock>>,
        pub oncopy: Callback<Event>,
    }

    pub enum TextBlockMessage {
        RemoveFocus,
        EnableContentEditing,
        BeginDrag(i32, i32),
        UpdateDrag(i32, i32),
        EndDrag,
    }

    pub struct TextBlock {
        contenteditable: bool,
        drag: Option<Drag>,
        node_ref: NodeRef,
        should_be_focused: bool,

        begin_drag: Callback<MouseEvent>,
        commit_lines: Callback<FocusEvent>,
        ondblclick: Callback<MouseEvent>,
        onmouseleave: Callback<MouseEvent>,
        onmousemove: Callback<MouseEvent>,
    }

    impl Component for TextBlock {
        type Properties = Props;
        type Message = TextBlockMessage;
        fn create(ctx: &Context<Self>) -> Self {
            let Props { block, commit_block, .. } = ctx.props();

            let commit_lines = {
                let block = block.clone();
                let commit = commit_block.clone();
                let link = ctx.link().clone();
                commit.reform(Self::new_commit_lines(link, block))
            };
            let ondblclick =
                ctx.link().callback(|_: MouseEvent| Self::Message::EnableContentEditing);
            let onmouseleave =
                ctx.link().callback(|_: MouseEvent| Self::Message::EndDrag);
            let onmousemove = ctx.link().callback(|e: MouseEvent| {
                Self::Message::UpdateDrag(e.client_x(), e.client_y())
            });
            let begin_drag = ctx.link().callback(|e: MouseEvent| {
                Self::Message::BeginDrag(e.client_x(), e.client_y())
            });

            Self {
                contenteditable: false,
                drag: None,
                node_ref: NodeRef::default(),
                should_be_focused: false,
                begin_drag,
                commit_lines,
                ondblclick,
                onmouseleave,
                onmousemove,
            }
        }

        fn changed(&mut self, ctx: &Context<Self>, previous: &Self::Properties) -> bool {
            let Props { block, commit_block, .. } = ctx.props();
            if block != &previous.block || commit_block != &previous.commit_block {
                self.commit_lines = {
                    let block = block.clone();
                    let commit = commit_block.clone();
                    let link = ctx.link().clone();
                    commit.reform(Self::new_commit_lines(link, block))
                };
            }
            true
        }

        fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
            match msg {
                Self::Message::RemoveFocus => {
                    self.should_be_focused = false;
                    if self.contenteditable {
                        self.contenteditable = false;
                        true
                    } else { false }
                }
                Self::Message::EnableContentEditing => {
                    self.contenteditable = true;
                    true
                }
                Self::Message::BeginDrag(x, y) => {
                    self.should_be_focused = true;
                    self.drag = Some(Drag::new(x, y));
                    true
                }
                Self::Message::UpdateDrag(x, y) => {
                    if let Some(drag) = self.drag {
                        self.drag = Some(drag.move_to(x, y));
                        true
                    } else { false }
                }
                Self::Message::EndDrag => {
                    if self.drag.is_some() {
                        self.drag = None;
                        true
                    } else { false }
                }
            }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            let Props {
                bbox,
                block,
                mutable,
                scale,
                backspace_delete,
                commit_block,
                oncopy,
                ..
            } = ctx.props();
            if self.should_be_focused { crate::utils::web::focus(&self.node_ref); }
            // let focused = crate::utils::web::is_focused(&self.node_ref);
            let style = self.style(bbox, block, *scale);

            let onmouseup = {
                let bbox = bbox.clone();
                let block = block.clone();
                let link = ctx.link().clone();
                let node = self.node_ref.clone();
                let scale = *scale;
                commit_block.reform(move |_| {
                    let element = node.cast::<web_sys::Element>()
                        .expect_throw("could not resolve node reference");
                    let rect = element.get_bounding_client_rect();

                    let left = ((rect.left() - bbox.rect.left) * scale).round();
                    let right = (rect.width() * scale).round() + left;
                    let top = ((rect.top() - bbox.rect.top) * scale).round();
                    let bottom = (rect.height() * scale).round() + top;
                    let box_ = (left as u32, top as u32, right as u32, bottom as u32);

                    link.send_message(Self::Message::EndDrag);
                    if box_ == block.box_ { return None; }
                    Some(OcrBlock {
                        uuid: block.uuid.clone(),
                        box_,
                        vertical: block.vertical,
                        font_size: block.font_size,
                        lines: block.lines.clone(),
                    })
                })
            };


            let contenteditable = self.contenteditable.then(|| "true");
            let noop = Callback::noop();
            let onblur =
                if self.contenteditable { &self.commit_lines } else { &noop };
            let no_bubble = Callback::from(|e: KeyboardEvent| e.set_cancel_bubble(true));
            let noop = Callback::noop();
            let onkeydown =
                if *mutable && !self.contenteditable { backspace_delete } else { &noop };
            let onkeypress =
                if self.contenteditable { &no_bubble } else { &noop };
            let noop = Callback::noop();
            let onmousedown =
                if *mutable && !self.contenteditable { &self.begin_drag } else { &noop };
            let onmousemove =
                if self.drag.is_some() { &self.onmousemove } else { &noop };
            html! {
                <div
                  ref={&self.node_ref}
                  key={block.uuid.as_str()}
                  class={"ocr-block"}
                  {contenteditable} {style} tabindex={"-1"}
                  {onblur}
                  {oncopy}
                  ondblclick={&self.ondblclick}
                  {onkeydown} {onkeypress} {onmouseup} {onmousedown} {onmousemove}
                  onmouseleave={&self.onmouseleave}
                >
                    {block.lines.iter().map(|line| html!{<p>{line}</p>}).collect::<Html>()}
                </div>
            }
        }
    }

    impl TextBlock {
        fn new_commit_lines(link: Scope<Self>, block: OcrBlock) -> impl Fn(FocusEvent) -> Option<OcrBlock> {
            move |e: FocusEvent| {
                link.send_message(TextBlockMessage::RemoveFocus);
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
                (lines != block.lines).then(
                    || OcrBlock { uuid: block.uuid.clone(), lines, ..block }
                )
            }
        }

        fn style(&self, bbox: &BoundingBox, block: &OcrBlock, scale: f64) -> String {
            let mut s = String::new();

            let dx = self.drag.as_ref().map_or(0, |d: &Drag| d.delta_x()) as f64;
            let dy = self.drag.as_ref().map_or(0, |d: &Drag| d.delta_y()) as f64;

            let top = bbox.rect.top + ((block.box_.1 as f64) / scale) + dy;
            let left = bbox.rect.left + ((block.box_.0 as f64) / scale) + dx;
            let height = ((block.box_.3 - block.box_.1) as f64) / scale;
            let width = ((block.box_.2 - block.box_.0) as f64) / scale;

            if block.vertical {
                let right = bbox.screen.width - left - width;
                s.push_str(&format!("top: {top:.2}px; right: {right:.2}px; "));
            } else {
                s.push_str(&format!("top: {top:.2}px; left: {left:.2}px; "));
            };

            let max_height = (bbox.rect.height + bbox.rect.top - top).floor();
            let max_width = (bbox.rect.width + bbox.rect.left - left).floor();
            s.push_str(&format!(
                "height: {height:.2}px; width: {width:.2}px; \
         max-height: {max_height}px; max-width: {max_width}px; "
            ));

            let font = (block.font_size as f64) / scale;
            let mode = if block.vertical { "vertical-rl" } else { "horizontal-tb" };
            s.push_str(&format!("font-size: {font:.1}px; writing-mode: {mode}; "));
            s
        }
    }

    mod drag {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Drag {
            start_x: i32,
            start_y: i32,
            pos_x: i32,
            pos_y: i32,
        }

        impl Drag {
            pub fn new(x: i32, y: i32) -> Self {
                Self { start_x: x, start_y: y, pos_x: x, pos_y: y }
            }

            pub fn move_to(self, x: i32, y: i32) -> Self {
                Self { start_x: self.start_x, start_y: self.start_y, pos_x: x, pos_y: y }
            }

            pub fn delta_x(&self) -> i32 {
                self.pos_x - self.start_x
            }

            pub fn delta_y(&self) -> i32 {
                self.pos_y - self.start_y
            }
        }
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


mod window {
    use yew::NodeRef;

    use crate::utils::web::get_screen_size;

    #[derive(Copy, Clone, Default, PartialEq)]
    pub struct WindowState {
        pub screen: Screen,
        pub left: BoundingBox,
        pub right: BoundingBox,
    }

    impl WindowState {
        pub fn new(left: Rect, right: Rect) -> Self {
            let screen = Screen::default();
            let left = BoundingBox { rect: left, screen: screen.clone() };
            let right = BoundingBox { rect: right, screen: screen.clone() };
            Self { screen, left, right }
        }
    }

    #[derive(Copy, Clone, Default, PartialEq)]
    pub struct BoundingBox {
        pub rect: Rect,
        pub screen: Screen,
    }


    #[derive(Copy, Clone, PartialEq)]
    pub struct Screen {
        pub width: f64,
        pub height: f64,
    }

    impl Default for Screen {
        fn default() -> Self {
            let (width, height) = get_screen_size();
            Self { width, height }
        }
    }

    #[derive(Copy, Clone, Default, PartialEq)]
    pub struct Rect {
        pub top: f64,
        pub left: f64,
        pub bottom: f64,
        pub right: f64,
        pub height: f64,
        pub width: f64,
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
}
