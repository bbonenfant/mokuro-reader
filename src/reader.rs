use std::rc::Rc;

use enclose::enclose;
use rexie::Rexie;
use wasm_bindgen::UnwrapThrowExt;
use web_sys::{Event, HtmlElement, KeyboardEvent, MouseEvent};
use yew::{Callback, Component, Context, html, Html, NodeRef, Properties};

use crate::models::VolumeMetadata;
use crate::reader::window::{Rect, WindowState};
use crate::utils::{
    db::{get_volume, put_volume},
    timestamp,
    web::{focus, focused_element, window},
};

#[derive(Default)]
pub struct Cursor {
    pub magnify: bool,
    pub force: u64,
    pub position: (i32, i32),
}

#[derive(Properties, PartialEq)]
pub struct ReaderProps {
    pub db: Rc<Rexie>,
    pub volume_id: u32,
}

pub enum ReaderMessage {
    Set(VolumeMetadata),
    Focus,
    HelpToggle,
    MagnifierToggle,
    MutableToggle,
    NextPage,
    PrevPage,
    Resize(bool),
    UpdateCursor(i32, i32),
}

pub struct Reader {
    cursor: Cursor,
    mutable: bool,
    node: NodeRef,
    node_left: NodeRef,
    node_right: NodeRef,
    volume: Option<VolumeMetadata>,
    window: WindowState,
    show_help: bool,

    focus: Callback<()>,
    handle_keypress: Callback<KeyboardEvent>,
    handle_image_load: Callback<Event>,
    handle_right_click: Callback<MouseEvent>,
    update_cursor: Callback<MouseEvent>,
    _resize_listener: gloo_events::EventListener,
}

impl Component for Reader {
    type Message = ReaderMessage;
    type Properties = ReaderProps;

    fn create(ctx: &Context<Self>) -> Self {
        let _resize_listener = {
            let link = ctx.link().clone();
            gloo_events::EventListener::new_with_options(
                &window(),
                "resize",
                gloo_events::EventListenerOptions::enable_prevent_default(),
                move |_: &Event| link.send_message(Self::Message::Resize(false)),
            )
        };

        let focus = ctx.link().callback(|()| Self::Message::Focus);
        let handle_keypress = ctx.link().batch_callback(
            |e: KeyboardEvent| {
                // gloo_console::log!("KeyCode:", e.code());
                match e.code().as_str() {
                    "KeyE" => Some(Self::Message::MutableToggle),
                    "KeyH" => Some(Self::Message::HelpToggle),
                    "KeyX" => Some(Self::Message::PrevPage),
                    "KeyZ" => Some(Self::Message::NextPage),
                    _ => None
                }
            }
        );
        let handle_image_load =
            ctx.link().callback(|_: Event| Self::Message::Resize(true));
        let handle_right_click = ctx.link().callback(|e: MouseEvent| {
            e.prevent_default();
            Self::Message::MagnifierToggle
        });

        let update_cursor = ctx.link().callback(
            |e: MouseEvent| Self::Message::UpdateCursor(e.x(), e.y())
        );

        let cursor = Cursor::default();
        let window = WindowState::default();
        Self {
            cursor,
            mutable: false,
            node: NodeRef::default(),
            node_left: NodeRef::default(),
            node_right: NodeRef::default(),
            volume: None,
            window,
            show_help: false,
            focus,
            handle_keypress,
            handle_image_load,
            handle_right_click,
            update_cursor,
            _resize_listener,
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            let ReaderProps { db, volume_id } = ctx.props();
            ctx.link().send_future(enclose!((db, volume_id) async move {
                let volume = get_volume(&db, volume_id).await
                    .expect_throw("failed to get volume from IndexedDB");
                Self::Message::Set(volume)
            }))
        } else { gloo_console::debug!("Rerender Reader") }

        let element = focused_element();
        if element.is_none() || element.is_some_and(|elm| elm.tag_name() == "BODY") {
            if let Some(elm) = self.node.cast::<HtmlElement>() {
                gloo_console::info!("setting focus to #Reader");
                elm.focus().expect_throw("failed to focus #Reader");
            }
        }

        // On every rerender, check to see if the image proportions has changed.
        ctx.link().send_message(Self::Message::Resize(false));
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let ReaderProps { db, .. } = ctx.props();
        match msg {
            ReaderMessage::Set(volume) => {
                let previous = self.volume.replace(volume);
                previous != self.volume
            }
            ReaderMessage::Focus => {
                focus(&self.node);
                false
            }
            ReaderMessage::HelpToggle => {
                self.show_help = !self.show_help;
                true
            }
            ReaderMessage::MagnifierToggle => {
                self.cursor.magnify = !self.cursor.magnify;
                true
            }
            ReaderMessage::MutableToggle => {
                self.mutable = !self.mutable;
                true
            }
            ReaderMessage::NextPage => {
                if let Some(volume) = &mut self.volume {
                    volume.page_forward();
                    ctx.link().send_future(
                        enclose!((db, volume) Self::commit_volume(db, volume))
                    );
                }
                true
            }
            ReaderMessage::PrevPage => {
                if let Some(volume) = &mut self.volume {
                    volume.page_backward();
                    ctx.link().send_future(
                        enclose!((db, volume) Self::commit_volume(db, volume))
                    );
                }
                true
            }
            ReaderMessage::Resize(force) => {
                let left = Rect::try_from(&self.node_left).unwrap_or(self.window.left.rect);
                let right = Rect::try_from(&self.node_right).unwrap_or(self.window.right.rect);
                if left != self.window.left.rect || right != self.window.right.rect || force {
                    self.cursor.force = timestamp();
                    self.window = WindowState::new(left, right);
                    return true;
                }
                false
            }
            ReaderMessage::UpdateCursor(x, y) => {
                self.cursor.position = (x, y);
                self.cursor.magnify
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if let Some(volume) = &self.volume {
            let ReaderProps { db, volume_id } = ctx.props();
            let (page_right, page_left) = volume.select_pages();
            return html! {
                <div id="ReaderContainer">
                <div
                  ref={&self.node}
                  id="Reader"
                  class={self.mutable.then(||Some("editable"))}
                  tabindex="0"
                  oncontextmenu={&self.handle_right_click}
                  onkeypress={&self.handle_keypress}
                  onmousemove={&self.update_cursor}
                >
                if self.cursor.magnify {
                    <magnifier::Magnifier
                        cursor={self.cursor.position}
                        settings={volume.magnifier}
                        left={&self.node_left}
                        right={&self.node_right}
                        force={&self.cursor.force}
                    />
                }

                if let Some(name) = page_left {
                    <page::Page
                        {db}
                        {volume_id}
                        {name}
                        node_ref={&self.node_left}
                        bbox={self.window.left}
                        mutable={self.mutable}
                        onload={&self.handle_image_load}
                        focus_reader={&self.focus}
                    />
                }
                if let Some(name) = page_right {
                    <page::Page
                        {db}
                        {volume_id}
                        {name}
                        node_ref={&self.node_right}
                        bbox={self.window.right}
                        mutable={self.mutable}
                        onload={&self.handle_image_load}
                        focus_reader={&self.focus}
                    />
                }
                </div>
                if self.show_help {{help(self.mutable)}}
                </div>
            };
        }
        Html::default()
    }
}

fn help(editing: bool) -> Html {
    const HELP: &str =
        "H - Toggle Help | Z - Next Page | X - Previous Page | E - Toggle Editing ";
    const EDITING: &str =
        "\"-\" - Decrease Font | \"+\" - Increase Font | BACKSPACE - Delete Textbox";
    html! {
        <span id="HelpBanner">
            {if editing {format!("{HELP} | {EDITING}")} else {HELP.to_owned()} }
        </span>
    }
}

impl Reader {
    async fn commit_volume(db: Rc<Rexie>, volume: VolumeMetadata) -> ReaderMessage {
        let id = volume.id.expect_throw("missing volume id");
        gloo_console::log!(format!("updating volume ({id} - {})", volume.title));
        put_volume(&db, &volume).await
            .expect_throw("failed to update volume in IndexedDB");
        ReaderMessage::Set(volume)
    }
}

mod magnifier {
    use std::fmt::{Display, Formatter};

    use yew::{function_component, Html, html, NodeRef, use_mut_ref};
    use yew_autoprops::autoprops;

    use crate::models::MagnifierSettings;

    #[autoprops]
    #[function_component(Magnifier)]
    pub fn magnifier(
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
        html! {<div id="Magnifier" {style}/>}
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
}

mod page {
    use std::rc::Rc;

    use enclose::enclose;
    use rexie::Rexie;
    use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
    use web_sys::{ClipboardEvent, MouseEvent};
    use yew::{AttrValue, Callback, Component, Context, Event, Html, html, NodeRef, Properties};

    use crate::errors::AppError;
    use crate::models::{OcrBlock, PageImage, PageOcr};
    use crate::utils::db::{get_page_and_ocr, put_ocr};
    use crate::utils::web::get_selection;

    use super::drag::Drag;
    use super::window::BoundingBox;

    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub db: Rc<Rexie>,
        pub volume_id: u32,
        pub name: AttrValue,
        pub node_ref: NodeRef,
        pub bbox: BoundingBox,
        pub mutable: bool,
        pub onload: Callback<Event>,
        pub focus_reader: Callback<()>,
    }

    pub enum PageMessage {
        Set(PageImage, PageOcr),
        Refresh(bool),
        DeleteBlock(AttrValue),
        UpdateBlock(OcrBlock),
        BeginDrag(i32, i32),
        UpdateDrag(i32, i32),
        EndDrag,
    }

    pub struct Page {
        _url_object: Option<gloo_file::ObjectUrl>,
        drag: Option<Drag>,
        ocr: PageOcr,
        url: AttrValue,

        commit: Callback<OcrBlock>,
        delete: Callback<AttrValue>,
        begin_drag: Callback<MouseEvent>,
        end_drag: Callback<MouseEvent>,
        onmousemove: Callback<MouseEvent>,
        oncopy: Callback<Event>,
    }

    impl Component for Page {
        type Properties = Props;
        type Message = PageMessage;
        fn create(ctx: &Context<Self>) -> Self {
            let commit =
                ctx.link().callback(|block: OcrBlock| Self::Message::UpdateBlock(block));
            let delete =
                ctx.link().callback(|uuid: AttrValue| Self::Message::DeleteBlock(uuid));
            let begin_drag =
                ctx.link().callback(|e: MouseEvent| Self::Message::BeginDrag(e.x(), e.y()));
            let end_drag =
                ctx.link().callback(|_: MouseEvent| Self::Message::EndDrag);
            let onmousemove =
                ctx.link().callback(|e: MouseEvent| Self::Message::UpdateDrag(e.x(), e.y()));
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
                drag: None,
                ocr: PageOcr::default(),
                url: AttrValue::default(),
                commit,
                delete,
                begin_drag,
                end_drag,
                onmousemove,
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
                    ctx.props().focus_reader.emit(());
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
                PageMessage::BeginDrag(x, y) => {
                    self.drag = Some(Drag::new(x, y));
                    true
                }
                PageMessage::UpdateDrag(x, y) => {
                    if let Some(drag) = self.drag {
                        self.drag = Some(drag.move_to(x, y));
                        true
                    } else { false }
                }
                PageMessage::EndDrag => {
                    let drag = self.drag.take();
                    if let Some(drag) = drag.filter(|d| d.dirty()) {
                        // Prevent creating a new block from a click.
                        if !drag.dirty() { return true; }

                        let Props { bbox, db, name, volume_id, .. } = ctx.props();
                        let block = create_block(&drag, bbox, self.scale(bbox));
                        self.ocr.blocks.push(block);

                        let ocr = self.ocr.clone();
                        ctx.link().send_future(enclose!(
                            (db, volume_id => id, name) Self::commit_ocr(db, id, name, ocr)
                        ));
                        true
                    } else { false }
                }
            }
        }

        fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
            if first_render {
                let Props { db, volume_id, name, .. } = ctx.props();
                ctx.link().send_future(enclose!(
                    (db, volume_id => id, name) Self::fetch(db, id, name)
                ))
            } else { gloo_console::debug!("Rerender", ctx.props().name.as_str()) }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            if self._url_object.is_none() {
                return Html::default();
            }

            let Props { bbox, node_ref, onload, mutable, .. } = ctx.props();
            let draggable = Some("false");
            let src = &self.url;
            let scale = self.scale(bbox);

            let noop = Callback::noop();
            let dragging = *mutable && self.drag.is_some();
            let onmousedown = if *mutable { &self.begin_drag } else { &noop };
            let onmouseup = if *mutable { &self.end_drag } else { &noop };
            let onmousemove = if dragging { &self.onmousemove } else { &noop };
            let onmouseout = if dragging { &self.end_drag } else { &noop };

            let new_block = if let Some(drag) = &self.drag.filter(|d| d.dirty()) {
                let style = format!(
                    "top: {}px; left: {}px; height: {}px; width: {}px;",
                    drag.top(), drag.left(), drag.delta_y().abs(), drag.delta_x().abs()
                );
                html! { <div class="new-ocr-block" {style}/> }
            } else { Html::default() };

            html! {
                <>
                <img
                  ref={node_ref} class="reader-image"
                  {draggable} {src}
                  {onload} {onmousedown} {onmouseup} {onmousemove} {onmouseout}
                />
                {new_block}
                {
                    self.ocr.blocks.iter().map(|block| {
                        html!{ <super::ocr::TextBlock
                            key={block.uuid.as_str()}
                            {mutable}
                            bbox={*bbox}
                            {scale}
                            block={block.clone()}
                            commit_block={&self.commit}
                            delete_block={&self.delete}
                            oncopy={&self.oncopy}
                        /> }
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
            gloo_console::log!(format!("updating OCR ({id}, {name})"));
            let key = js_sys::Array::of2(&id.into(), &name.as_str().into());
            put_ocr(&db, &ocr, &key).await.unwrap_or_else(|error| {
                if let AppError::RexieError(err) = error {
                    gloo_console::error!(JsValue::from(err));
                }
            });
            PageMessage::Refresh(true)
        }

        #[inline(always)]
        fn scale(&self, bbox: &BoundingBox) -> f64 {
            (self.ocr.img_height as f64) / bbox.rect.height
        }
    }

    fn create_block(drag: &Drag, bbox: &BoundingBox, scale: f64) -> OcrBlock {
        let clamp = 26f64;
        let font_size = 20u32;

        let (drag_top, drag_left) = (drag.top() as f64, drag.left() as f64);
        let (drag_width, drag_height) = (drag.delta_x().abs() as f64, drag.delta_y().abs() as f64);

        let left = (drag_left - bbox.rect.left) * scale;
        let right = (drag_width * scale).max(clamp) + left;
        let top = (drag_top - bbox.rect.top) * scale;
        let bottom = (drag_height * scale).max(clamp) + top;
        let vertical = drag.delta_x() < 0;

        OcrBlock::new(top, left, bottom, right, font_size, vertical)
    }
}

mod ocr {
    use enclose::enclose;
    use wasm_bindgen::UnwrapThrowExt;
    use web_sys::{Event, FocusEvent, KeyboardEvent, MouseEvent};
    use yew::{AttrValue, Callback, Component, Context, Html, html, NodeRef, Properties};

    use crate::models::OcrBlock;
    use crate::utils::web::get_bounding_rect;

    use super::drag::Drag;
    use super::window::BoundingBox;

    const DELETE_PROMPT: &str = "Are you sure you want to delete this?\nThere is no undo!";

    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub bbox: BoundingBox,
        pub block: OcrBlock,
        pub mutable: bool,
        pub scale: f64,

        pub commit_block: Callback<OcrBlock>,
        pub delete_block: Callback<AttrValue>,
        pub oncopy: Callback<Event>,
    }

    pub enum TextBlockMessage {
        RemoveFocus,
        EnableContentEditing,
        IncreaseFontSize,
        DecreaseFontSize,
        BeginDrag(i32, i32),
        UpdateDrag(i32, i32),
        EndDrag,
        CommitLines,
        DeleteBlock,
    }

    pub struct TextBlock {
        contenteditable: bool,
        drag: Option<Drag>,
        node_ref: NodeRef,
        should_be_focused: bool,

        begin_drag: Callback<MouseEvent>,
        commit_lines: Callback<FocusEvent>,
        handle_keypress: Callback<KeyboardEvent>,
        ondblclick: Callback<MouseEvent>,
        onmouseleave: Callback<MouseEvent>,
        onmousemove: Callback<MouseEvent>,
        remove_focus: Callback<FocusEvent>,
    }

    impl Component for TextBlock {
        type Properties = Props;
        type Message = TextBlockMessage;
        fn create(ctx: &Context<Self>) -> Self {
            let begin_drag =
                ctx.link().callback(|e: MouseEvent| Self::Message::BeginDrag(e.x(), e.y()));
            let commit_lines =
                ctx.link().callback(|_: FocusEvent| Self::Message::CommitLines);
            let handle_keypress = ctx.link().batch_callback(|e: KeyboardEvent| {
                match e.code().as_str() {
                    "Backspace" => {
                        if gloo_dialogs::confirm(DELETE_PROMPT) {
                            Some(Self::Message::DeleteBlock)
                        } else { None }
                    }
                    "Minus" => Some(Self::Message::DecreaseFontSize),
                    "Equal" => Some(Self::Message::IncreaseFontSize),
                    _ => None,
                }
            });
            let ondblclick =
                ctx.link().callback(|_: MouseEvent| Self::Message::EnableContentEditing);
            let onmouseleave =
                ctx.link().callback(|_: MouseEvent| Self::Message::EndDrag);
            let onmousemove =
                ctx.link().callback(|e: MouseEvent| Self::Message::UpdateDrag(e.x(), e.y()));
            let remove_focus =
                ctx.link().callback(|_: FocusEvent| Self::Message::RemoveFocus);

            Self {
                contenteditable: false,
                drag: None,
                node_ref: NodeRef::default(),
                should_be_focused: false,
                begin_drag,
                commit_lines,
                handle_keypress,
                ondblclick,
                onmouseleave,
                onmousemove,
                remove_focus,
            }
        }

        fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
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
                Self::Message::IncreaseFontSize => {
                    let mut block = ctx.props().block.clone();
                    block.font_size += 1;
                    ctx.props().commit_block.emit(block);
                    false
                }
                Self::Message::DecreaseFontSize => {
                    let mut block = ctx.props().block.clone();
                    block.font_size -= 1;
                    ctx.props().commit_block.emit(block);
                    false
                }
                Self::Message::BeginDrag(x, y) => {
                    self.should_be_focused = true;
                    self.drag = Some(Drag::new(x, y));
                    true
                }
                Self::Message::UpdateDrag(x, y) => {
                    if let Some(drag) = self.drag {
                        // There is not a way to differentiate at the time of click
                        // whether the mouse is clicking on the text box <div> or the
                        // resize handler. Therefore, we treat every click like it's
                        // potentially a drag, but if the dimensions of the <div> has
                        // changed, we abort the drag update and let the browser
                        // handle the resize.
                        let Props { bbox, block, scale, .. } = ctx.props();
                        let rect = get_bounding_rect(&self.node_ref);
                        if ((rect.height() * scale) - block.height()).abs() >= 0.1
                            || ((rect.width() * scale) - block.width()).abs() >= 0.1 {
                            // using 0.1 is arbitrary and might be problematic.  ^
                            self.drag = None;
                            return true;
                        }

                        // Ensure that the block is not dragged outside the image.
                        // We stay 1px away from the image border to avoid edge cases.
                        let drag = {
                            let new = drag.move_x(x);
                            let dx = (new.delta_x() - drag.delta_x()) as f64;
                            if (rect.left() + dx) < (bbox.rect.left + 1.)
                                || (rect.right() + dx) > (bbox.rect.right - 1.)
                            { drag } else { new }
                        };
                        let drag = {
                            let new = drag.move_y(y);
                            let dy = (new.delta_y() - drag.delta_y()) as f64;
                            if (rect.top() + dy) < (bbox.rect.top + 1.)
                                || (rect.bottom() + dy) > (bbox.rect.bottom - 1.)
                            { drag } else { new }
                        };
                        self.drag = Some(drag);
                        true
                    } else { false }
                }
                Self::Message::EndDrag => {
                    if self.drag.is_some() {
                        self.drag = None;
                        true
                    } else { false }
                }
                Self::Message::CommitLines => {
                    let mut block = ctx.props().block.clone();
                    let children = self.html_element().children();
                    let lines: Vec<AttrValue> = (0..children.length()).filter_map(|idx|
                    children.item(idx).unwrap().text_content().map(|t| t.into())
                    ).collect();
                    if lines != block.lines {
                        block.lines = lines;
                        ctx.props().commit_block.emit(block);
                    }
                    ctx.link().send_message(TextBlockMessage::RemoveFocus);
                    false
                }
                Self::Message::DeleteBlock => {
                    let Props { block, delete_block, .. } = ctx.props();
                    delete_block.emit(block.uuid.clone());
                    false
                }
            }
        }

        fn rendered(&mut self, _ctx: &Context<Self>, _first_render: bool) {
            if self.should_be_focused {
                crate::utils::web::focus(&self.node_ref);
            }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            let Props {
                bbox,
                block,
                mutable,
                scale,
                commit_block,
                oncopy,
                ..
            } = ctx.props();
            let style = self.style(bbox, block, *scale);

            let node = self.node_ref.clone();
            let onmouseup = enclose!((bbox, block, commit_block, scale)
                ctx.link().callback(move |_: MouseEvent| {
                    let rect = get_bounding_rect(&node);
                    let left = ((rect.left() - bbox.rect.left) * scale).round();
                    let right = (rect.width() * scale).round() + left;
                    let top = ((rect.top() - bbox.rect.top) * scale).round();
                    let bottom = (rect.height() * scale).round() + top;
                    let box_ = (left as u32, top as u32, right as u32, bottom as u32);
                    if box_ != block.box_ {
                        let mut block = block.clone();
                        block.box_ = box_;
                        commit_block.emit(block.to_owned());
                    }
                    Self::Message::EndDrag
                })
            );

            let onblur =
                if self.contenteditable { &self.commit_lines } else { &self.remove_focus };
            let no_bubble = Callback::from(|e: KeyboardEvent| e.set_cancel_bubble(true));
            let noop = Callback::noop();
            let onkeydown =
                if *mutable && !self.contenteditable { &self.handle_keypress } else { &noop };
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
                  contenteditable={self.contenteditable.then(|| "true")}
                  {style} tabindex={"-1"}
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
        fn html_element(&self) -> web_sys::HtmlElement {
            self.node_ref.cast::<web_sys::HtmlElement>()
                .expect_throw("could not resolve node reference")
        }

        fn style(&self, bbox: &BoundingBox, block: &OcrBlock, scale: f64) -> String {
            let mut s = String::new();

            let dx = self.drag.as_ref().map_or(0, |d: &Drag| d.delta_x()) as f64;
            let dy = self.drag.as_ref().map_or(0, |d: &Drag| d.delta_y()) as f64;

            let top = bbox.rect.top + (block.top() / scale) + dy;
            let left = bbox.rect.left + (block.left() / scale) + dx;
            let height = block.height() / scale;
            let width = block.width() / scale;

            if block.vertical {
                let right = bbox.screen.width - left - width;
                s.push_str(&format!("top: {top:.2}px; right: {right:.2}px; "));
            } else {
                s.push_str(&format!("top: {top:.2}px; left: {left:.2}px; "));
            };

            let max_height = (bbox.rect.height + bbox.rect.top - top).floor();
            let max_width = if block.vertical {
                (left + width - bbox.rect.left).floor()
            } else {
                (bbox.rect.right - left).floor()
            };
            s.push_str(&format!(
                "height: {height:.2}px; width: {width:.2}px; \
                 max-height: {max_height}px; max-width: {max_width}px; "
            ));

            let font = (block.font_size as f64) / scale;
            let mode = if block.vertical { "vertical-rl" } else { "horizontal-tb" };
            s.push_str(&format!("font-size: {font:.1}px; writing-mode: {mode}; "));
            s.push_str("line-height: 1.5; ");
            if self.drag.is_some_and(|d| d.dirty()) {
                s.push_str("opacity: 50%; ");
            }
            s
        }
    }
}

mod drag {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Drag {
        start_x: i32,
        start_y: i32,
        pos_x: i32,
        pos_y: i32,
        dirty: bool,
    }

    impl Drag {
        pub fn new(x: i32, y: i32) -> Self {
            Self { start_x: x, start_y: y, pos_x: x, pos_y: y, dirty: false }
        }

        pub fn move_to(self, x: i32, y: i32) -> Self {
            Self {
                start_x: self.start_x,
                start_y: self.start_y,
                pos_x: x,
                pos_y: y,
                dirty: self.dirty || (self.start_x != x) || (self.start_y != y),
            }
        }

        pub fn move_x(&self, x: i32) -> Self {
            Self {
                start_x: self.start_x,
                start_y: self.start_y,
                pos_x: x,
                pos_y: self.pos_y,
                dirty: self.dirty || (self.start_x != x),
            }
        }

        pub fn move_y(&self, y: i32) -> Self {
            Self {
                start_x: self.start_x,
                start_y: self.start_y,
                pos_x: self.pos_x,
                pos_y: y,
                dirty: self.dirty || (self.start_y != y),
            }
        }

        pub fn delta_x(&self) -> i32 {
            self.pos_x - self.start_x
        }

        pub fn delta_y(&self) -> i32 {
            self.pos_y - self.start_y
        }

        pub fn left(&self) -> i32 { self.pos_x.min(self.start_x) }
        pub fn top(&self) -> i32 { self.pos_y.min(self.start_y) }
        pub fn dirty(&self) -> bool { self.dirty }
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
