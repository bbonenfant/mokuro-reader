use std::rc::Rc;

use enclose::enclose;
use rexie::Rexie;
use wasm_bindgen::UnwrapThrowExt;
use web_sys::{Event, HtmlElement, KeyboardEvent, MouseEvent};
use yew::{html, Callback, Component, Context, Html, NodeRef, Properties};

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
    Commit(sidebar::SidebarData),
    Focus,
    HelpToggle,
    MagnifierToggle,
    MutableToggle,
    SidebarToggle,
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
    sidebar_expanded: bool,

    commit_sidebar_data: Callback<sidebar::SidebarData>,
    focus: Callback<()>,
    handle_keypress: Callback<KeyboardEvent>,
    handle_image_load: Callback<Event>,
    handle_right_click: Callback<MouseEvent>,
    toggle_sidebar: Callback<MouseEvent>,
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

        let commit_sidebar_data =
            ctx.link().callback(|data| Self::Message::Commit(data));
        let focus = ctx.link().callback(|()| Self::Message::Focus);
        let handle_keypress = ctx.link().batch_callback(
            |e: KeyboardEvent| {
                // gloo_console::log!("KeyCode:", e.code());
                match e.code().as_str() {
                    "KeyE" => Some(Self::Message::MutableToggle),
                    "KeyH" => Some(Self::Message::HelpToggle),
                    "KeyS" => Some(Self::Message::SidebarToggle),
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
        let toggle_sidebar = ctx.link().callback(|e: MouseEvent| {
            e.prevent_default();
            Self::Message::SidebarToggle
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
            sidebar_expanded: false,
            commit_sidebar_data,
            focus,
            handle_keypress,
            handle_image_load,
            handle_right_click,
            toggle_sidebar,
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
        }

        let element = focused_element();
        if element.is_none() || element.is_some_and(|elm| elm.tag_name() == "BODY") {
            if let Some(elm) = self.node.cast::<HtmlElement>() {
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
            ReaderMessage::Commit(data) => {
                let sidebar::SidebarData {
                    first_page_is_cover,
                    hide_sidebar,
                    line_height,
                    magnifier_height,
                    magnifier_width,
                    magnifier_radius,
                    magnification,
                    show_help,
                    show_magnifier,
                } = data;
                self.show_help = show_help;
                if show_magnifier && !self.cursor.magnify {
                    self.cursor.position = (
                        (self.window.screen.width / 2.0) as i32,
                        (self.window.screen.height / 2.0) as i32,
                    );
                }
                self.cursor.magnify = show_magnifier;
                if let Some(volume) = &mut self.volume {
                    volume.hide_sidebar = hide_sidebar;
                    volume.line_height = line_height;
                    volume.magnifier.height = magnifier_height;
                    volume.magnifier.width = magnifier_width;
                    volume.magnifier.radius = magnifier_radius;
                    volume.magnifier.zoom = magnification;
                    volume.reader_state.first_page_is_cover = first_page_is_cover;
                    ctx.link().send_future(
                        enclose!((db, volume) Self::commit_volume(db, volume))
                    );
                }
                true
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
            ReaderMessage::SidebarToggle => {
                self.sidebar_expanded = !self.sidebar_expanded;
                if !self.sidebar_expanded {
                    focus(&self.node);
                }
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
            let magnifier = if self.cursor.magnify {
                volume.magnifier.render(&self.cursor.position, &self.node_left, &self.node_right)
            } else { Html::default() };
            return html! {
            <div id="ReaderGrid" tabindex={"-1"} onkeypress={&self.handle_keypress}>
                <sidebar::Sidebar
                  commit={&self.commit_sidebar_data}
                  onblur={&self.focus}
                  toggle_sidebar={&self.toggle_sidebar}
                  expanded={self.sidebar_expanded}
                  data={sidebar::SidebarData{
                    first_page_is_cover: volume.reader_state.first_page_is_cover,
                    hide_sidebar: volume.hide_sidebar,
                    line_height: volume.line_height,
                    magnifier_width: volume.magnifier.width,
                    magnifier_height: volume.magnifier.height,
                    magnifier_radius: volume.magnifier.radius,
                    magnification: volume.magnifier.zoom,
                    show_help: self.show_help,
                    show_magnifier: self.cursor.magnify,
                  }}
                />
                <div
                  ref={&self.node}
                  id="Reader"
                  class={self.mutable.then(||Some("editable"))}
                  style={format!("line-height: {:.1}", volume.line_height)}
                  tabindex="-1"
                  oncontextmenu={&self.handle_right_click}
                  onmousemove={&self.update_cursor}
                >
                {pagebar(
                    self.window.left.rect.height as u32,
                    ctx.link().callback(|_| Self::Message::NextPage),
                )}

                {magnifier}

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

                {pagebar(
                    self.window.right.rect.height as u32,
                    ctx.link().callback(|_| Self::Message::PrevPage),
                )}

                </div>
                if self.show_help {{help(self.mutable)}}
            </div>
            };
        }
        Html::default()
    }
}

fn pagebar(
    height: u32,
    move_page: Callback<MouseEvent>,
) -> Html {
    let style = format!("height: {height}px; ");
    html! {
        <div class="pagebar" {style}>
            <button onclick={move_page}>
                {crate::icons::chevron()}
            </button>
        </div>
    }
}

fn help(editing: bool) -> Html {
    const HELP: &str =
        "H - Toggle Help | Z - Next Page | X - Previous Page | E - Toggle Editing | S - Toggle Sidebar | Right Click - Toggle Magnifier";
    const EDITING: &str =
        "\"-\" - Decrease Font | \"+\" - Increase Font | 0 - Autosize Box to Text |  \"\\\" - Toggle Text Opacity | BACKSPACE - Delete Textbox";
    html! {
        <span id="HelpBanner">
            {if editing {format!("{HELP} || {EDITING}")} else {HELP.to_owned()} }
        </span>
    }
}

impl Reader {
    async fn commit_volume(db: Rc<Rexie>, volume: VolumeMetadata) -> ReaderMessage {
        // gloo_console::log!(format!("updating volume ({id} - {})", volume.title));
        put_volume(&db, &volume).await
            .expect_throw("failed to update volume in IndexedDB");
        ReaderMessage::Set(volume)
    }
}

mod magnifier {
    impl crate::models::MagnifierSettings {
        pub(crate) fn render(
            &self, cursor: &(i32, i32), left_ref: &yew::NodeRef, right_ref: &yew::NodeRef,
        ) -> yew::Html {
            let no_magnifier = yew::Html::default();
            let (zoom, height, width, radius) =
                (self.zoom as i32, self.height as i32, self.width as i32, self.radius);

            // The node refs may not resolve to a currently rendered HTML elements,
            // i.e. in the case where only one page is being displayed instead of two.
            // If neither node ref is valid, then there's no image to magnify, and so
            // we exit early.
            let left_img = left_ref.cast::<web_sys::Element>();
            let right_img = right_ref.cast::<web_sys::Element>();
            let Some(img) = left_img.as_ref().or(right_img.as_ref()) else { return no_magnifier; };
            let single_page = left_img.is_some() ^ right_img.is_some();

            // Get some information about the image size and position.
            let (img_height, img_width, img_top, img_left) = {
                let rect = img.get_bounding_client_rect();
                (rect.height() as i32, rect.width() as i32, rect.top() as i32, rect.left() as i32)
            };
            if img_height == 0 || img_width == 0 { return no_magnifier; }

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

            let background = {
                // css format: url() position / size repeat
                let left_ = left_img.map(|element| {
                    let url = element.get_attribute("src").unwrap();
                    format!("url({url}) {x_shift}px {y_shift}px / {z_width}px {z_height}px no-repeat")
                });
                let right_ = right_img.map(|element| {
                    let url = element.get_attribute("src").unwrap();
                    let x_shift = if single_page { x_shift } else {
                        center_x - (((x - img_width) * zoom) / 100)
                    };
                    format!("url({url}) {x_shift}px {y_shift}px / {z_width}px {z_height}px no-repeat")
                });
                match (left_, right_) {
                    (Some(l), Some(r)) => format!("background: {l}, {r}"),
                    (Some(l), None) => format!("background: {l}"),
                    (None, Some(r)) => format!("background: {r}"),
                    (None, None) => "".to_string(),
                }
            };

            // The position of the magnifier element.
            let left = img_left + x - center_x;
            let top = img_top + y - center_y;
            let style = format!("left: {left}px; top: {top}px; width: {width}px; height: {height}px; border-radius: {radius}%; {background}");
            yew::html! { <div id="Magnifier" {style}/> }
        }
    }
}

mod page {
    use std::rc::Rc;

    use enclose::enclose;
    use rexie::Rexie;
    use wasm_bindgen::{JsCast, UnwrapThrowExt};
    use web_sys::{ClipboardEvent, MouseEvent};
    use yew::{html, AttrValue, Callback, Component, Context, Event, Html, NodeRef, Properties};

    use crate::models::{OcrBlock, PageImage, PageOcr};
    use crate::utils::db::{get_page_and_ocr, put_ocr};
    use crate::utils::web::{focus, get_selection};

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
        ReportBlur(NodeRef),
        DeleteBlock(AttrValue),
        UpdateBlock(OcrBlock),
        BeginDrag(i32, i32),
        UpdateDrag(i32, i32),
        EndDrag,
    }

    pub struct Page {
        _url_object: Option<gloo_file::ObjectUrl>,
        drag: Option<Drag>,
        last_focus: Option<NodeRef>,
        ocr: PageOcr,
        url: AttrValue,

        commit: Callback<OcrBlock>,
        delete: Callback<AttrValue>,
        begin_drag: Callback<MouseEvent>,
        end_drag: Callback<MouseEvent>,
        onmousemove: Callback<MouseEvent>,
        oncopy: Callback<Event>,
        report_blur: Callback<NodeRef>,
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
            let report_blur =
                ctx.link().callback(|node| Self::Message::ReportBlur(node));
            Self {
                _url_object: None,
                drag: None,
                last_focus: None,
                ocr: PageOcr::default(),
                url: AttrValue::default(),
                commit,
                delete,
                begin_drag,
                end_drag,
                onmousemove,
                oncopy,
                report_blur,
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
            true
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
                PageMessage::ReportBlur(node) => {
                    self.last_focus = Some(node);
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
                    if let Some(node) = &self.last_focus
                    { focus(node); } else { ctx.props().focus_reader.emit(()); }
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
            }
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
                            report_blur={&self.report_blur}
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
            let key = js_sys::Array::of2(&id.into(), &name.as_str().into());
            put_ocr(&db, &ocr, &key).await.unwrap_throw();
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
    use wasm_bindgen::{JsCast, UnwrapThrowExt};
    use web_sys::{Event, FocusEvent, KeyboardEvent, MouseEvent};
    use yew::{html, AttrValue, Callback, Component, Context, Html, NodeRef, Properties};

    use crate::models::OcrBlock;
    use crate::utils::timestamp;
    use crate::utils::web::{get_bounding_rect, set_caret};

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
        pub report_blur: Callback<NodeRef>,
    }

    pub enum TextBlockMessage {
        RemoveFocus,
        SetContentEditing(bool),
        ToggleTransparency,
        IncreaseFontSize,
        DecreaseFontSize,
        BeginDrag(i32, i32),
        UpdateDrag(i32, i32),
        EndDrag,
        Autosize,
        Move(Direction),
        CommitLines,
        DeleteBlock,
    }

    pub enum Direction {
        Up,
        Down,
        Left,
        Right,
    }

    pub struct TextBlock {
        contenteditable: bool,
        drag: Option<Drag>,
        node_ref: NodeRef,
        should_be_focused: bool,
        transparent: bool,
        stamp: u64,  // timestamp (used to force redraws)

        begin_drag: Callback<MouseEvent>,
        commit_lines: Callback<FocusEvent>,
        handle_escape: Callback<KeyboardEvent>,
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
            let handle_escape = ctx.link().batch_callback(|e: KeyboardEvent| {
                if e.code().as_str() == "Escape" {
                    vec![Self::Message::SetContentEditing(false), Self::Message::CommitLines]
                } else { vec![] }
            });
            let handle_keypress = ctx.link().batch_callback(|e: KeyboardEvent| {
                match e.code().as_str() {
                    "Backquote" => {
                        e.prevent_default();
                        Some(Self::Message::SetContentEditing(true))
                    }
                    "Backslash" => Some(Self::Message::ToggleTransparency),
                    "Backspace" => {
                        if gloo_dialogs::confirm(DELETE_PROMPT) {
                            Some(Self::Message::DeleteBlock)
                        } else { None }
                    }
                    "Minus" => Some(Self::Message::DecreaseFontSize),
                    "Equal" => Some(Self::Message::IncreaseFontSize),
                    "Digit0" => Some(Self::Message::Autosize),
                    "ArrowUp" => {
                        e.prevent_default();
                        Some(Self::Message::Move(Direction::Up))
                    }
                    "ArrowDown" => {
                        e.prevent_default();
                        Some(Self::Message::Move(Direction::Down))
                    }
                    "ArrowLeft" => {
                        e.prevent_default();
                        Some(Self::Message::Move(Direction::Left))
                    }
                    "ArrowRight" => {
                        e.prevent_default();
                        Some(Self::Message::Move(Direction::Right))
                    }
                    _ => None,
                }
            });
            let ondblclick =
                ctx.link().callback(|_: MouseEvent| Self::Message::SetContentEditing(true));
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
                transparent: false,
                stamp: timestamp(),
                begin_drag,
                commit_lines,
                handle_escape,
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
                    self.transparent = false;
                    if self.contenteditable {
                        self.contenteditable = false;
                    }
                    ctx.props().report_blur.emit(self.node_ref.clone());
                    true
                }
                Self::Message::SetContentEditing(value) => {
                    self.contenteditable = value;
                    if self.contenteditable { set_caret(&self.node_ref) }
                    true
                }
                Self::Message::ToggleTransparency => {
                    self.transparent = !self.transparent;
                    true
                }
                Self::Message::IncreaseFontSize => {
                    let mut block = ctx.props().block.clone();
                    block.font_size += 1;
                    ctx.props().commit_block.emit(block);
                    self.transparent = true;
                    false
                }
                Self::Message::DecreaseFontSize => {
                    let mut block = ctx.props().block.clone();
                    block.font_size -= 1;
                    ctx.props().commit_block.emit(block);
                    self.transparent = true;
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
                Self::Message::Autosize => {
                    let element = self.node_ref.cast::<web_sys::Element>()
                        .expect_throw("could not resolve node reference");
                    let (mut left, mut top, mut right, mut bottom) = {
                        let child = element.children().get_with_index(0).unwrap();
                        let bbox = child.get_bounding_client_rect();
                        (bbox.left(), bbox.top(), bbox.right(), bbox.bottom())
                    };
                    for idx in 1..element.child_element_count() {
                        let child = element.children().get_with_index(idx).unwrap();
                        let bbox = child.get_bounding_client_rect();
                        left = left.min(bbox.left());
                        top = top.min(bbox.top());
                        right = right.max(bbox.right());
                        bottom = bottom.max(bbox.bottom());
                    }

                    let Props {
                        bbox,
                        block,
                        commit_block,
                        scale,
                        ..
                    } = ctx.props();
                    let box_ = (
                        ((left - bbox.rect.left) * scale).round() as u32,
                        ((top - bbox.rect.top) * scale).round() as u32,
                        ((right - bbox.rect.left) * scale).round() as u32,
                        ((bottom - bbox.rect.top) * scale).round() as u32,
                    );
                    let mut block = block.clone();
                    block.box_ = box_;
                    commit_block.emit(block.to_owned());
                    true
                }
                Self::Message::Move(direction) => {
                    let Props { block, commit_block, .. } = ctx.props();
                    let mut box_ = block.box_.clone();
                    // box_ = (left as u32, top as u32, right as u32, bottom as u32)
                    match direction {
                        Direction::Up => {
                            if let Some(top) = box_.1.checked_sub(1) {
                                box_.1 = top;
                                box_.3 -= 1;
                            }
                        }
                        Direction::Down => {
                            box_.1 += 1;
                            box_.3 += 1;
                        }
                        Direction::Left => {
                            if let Some(left) = box_.0.checked_sub(1) {
                                box_.0 = left;
                                box_.2 -= 1;
                            }
                        }
                        Direction::Right => {
                            box_.0 += 1;
                            box_.2 += 1;
                        }
                    }

                    let mut block = block.clone();
                    block.box_ = box_;
                    commit_block.emit(block.to_owned());
                    self.transparent = true;
                    true
                }
                Self::Message::CommitLines => {
                    let mut block = ctx.props().block.clone();
                    let children = self.html_element().children();

                    // Grab the lines from only the <p> nodes. This may be overly restrictive.
                    // TODO: Check how browsers handle newlines in contenteditable nodes.
                    let mut lines: Vec<AttrValue> = (0..children.length())
                        .map(|idx| children.item(idx).unwrap())
                        .filter(|elm| elm.tag_name() == "P")
                        .filter_map(|elm| elm.text_content().map(|t| t.into()))
                        .collect();

                    // This is in case the user writes to the div node directly instead of
                    // writing to a <p> node. This can happen if the user deletes all
                    // the <p> nodes.
                    if let Some(node) = self.html_element().child_nodes().get(0) {
                        if node.node_type() == web_sys::Node::TEXT_NODE {
                            if let Some(text) = node.text_content() {
                                lines.insert(0, text.into())
                            }
                        }
                    }
                    if lines != block.lines || lines.is_empty() {
                        self.stamp = timestamp();  // Use this to force redraw.
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
                // Focus on the first <p> tag. This minimizes the chance that the
                // user will write text to the <div> tag.
                self.html_element().children().get_with_index(0).map(|elm|
                    elm.dyn_into::<web_sys::HtmlElement>().unwrap().focus().ok()
                );
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
                match (*mutable, self.contenteditable) {
                    (true, false) => &self.handle_keypress,
                    (true, true) => &self.handle_escape,
                    _ => &noop
                };
            let onkeypress =
                if self.contenteditable { &no_bubble } else { &noop };
            let noop = Callback::noop();
            let ondblclick =
                if *mutable && !self.contenteditable { &self.ondblclick } else { &noop };
            let onmousedown =
                if *mutable && !self.contenteditable { &self.begin_drag } else { &noop };
            let onmousemove =
                if self.drag.is_some() { &self.onmousemove } else { &noop };
            html! {
                <div
                  ref={&self.node_ref}
                  key={format!("{}-{}", block.uuid.as_str(), self.stamp)}
                  class={"ocr-block"}
                  contenteditable={self.contenteditable.then(|| "true")}
                  {style} tabindex={"0"}
                  {onblur} {oncopy} {ondblclick}
                  {onkeydown} {onkeypress} {onmouseup} {onmousedown} {onmousemove}
                  onmouseleave={&self.onmouseleave}
                >
                    {if block.lines.iter().all(|line| line.trim().is_empty()) {
                        html!{<p>{"placeholder"}</p>}
                    } else {
                        block.lines.iter().map(
                            |line| html!{<p>{line}</p>}
                        ).collect::<Html>()
                    }}
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
            if self.transparent || self.drag.is_some_and(|d| d.dirty()) {
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
                dirty: self.dirty || ((self.start_x - x).abs() > 2) || ((self.start_y - y).abs() > 2),
            }
        }

        pub fn move_x(&self, x: i32) -> Self {
            Self {
                start_x: self.start_x,
                start_y: self.start_y,
                pos_x: x,
                pos_y: self.pos_y,
                dirty: self.dirty || ((self.start_x - x).abs() > 2),
            }
        }

        pub fn move_y(&self, y: i32) -> Self {
            Self {
                start_x: self.start_x,
                start_y: self.start_y,
                pos_x: self.pos_x,
                pos_y: y,
                dirty: self.dirty || ((self.start_y - y).abs() > 2),
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

mod sidebar {
    use web_sys::{Event, FocusEvent, MouseEvent};
    use yew::{html, Callback, Component, Context, Html, NodeRef, Properties};
    use yew_router::prelude::Link;

    use crate::icons;
    use crate::utils::web::{get_input_bool, get_input_f64, get_input_u16, get_input_u8};
    use crate::Route;

    #[derive(Properties, PartialEq)]
    pub struct Props {
        pub data: SidebarData,
        pub expanded: bool,
        pub commit: Callback<SidebarData>,
        pub onblur: Callback<()>,
        pub toggle_sidebar: Callback<MouseEvent>,
    }

    #[derive(PartialEq)]
    pub struct SidebarData {
        pub first_page_is_cover: bool,
        pub hide_sidebar: bool,
        pub line_height: f64,
        pub magnifier_height: u16,
        pub magnifier_width: u16,
        pub magnifier_radius: u8,
        pub magnification: u16,

        pub show_help: bool,
        pub show_magnifier: bool,
    }

    pub struct Sidebar {
        onblur: Callback<FocusEvent>,
        onchange: Callback<Event>,

        // NodeRefs
        cover_toggle_ref: NodeRef,
        hide_toggle_ref: NodeRef,
        line_height_ref: NodeRef,
        magnifier_height_ref: NodeRef,
        magnifier_width_ref: NodeRef,
        magnifier_radius_ref: NodeRef,
        magnification_ref: NodeRef,
        show_help_ref: NodeRef,
        show_magnifier_ref: NodeRef,
    }

    pub enum Message {
        Commit
    }

    impl Component for Sidebar {
        type Message = Message;
        type Properties = Props;

        fn create(ctx: &Context<Self>) -> Self {
            let onblur = ctx.props().onblur.clone();
            let onblur = Callback::from(move |_| onblur.emit(()));
            let onchange = ctx.link().callback(|_| Message::Commit);
            Self {
                onblur,
                onchange,
                cover_toggle_ref: NodeRef::default(),
                hide_toggle_ref: NodeRef::default(),
                line_height_ref: NodeRef::default(),
                magnifier_height_ref: NodeRef::default(),
                magnifier_width_ref: NodeRef::default(),
                magnifier_radius_ref: NodeRef::default(),
                magnification_ref: NodeRef::default(),
                show_help_ref: NodeRef::default(),
                show_magnifier_ref: NodeRef::default(),
            }
        }

        fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
            let Props { commit, data, .. } = ctx.props();
            match msg {
                Message::Commit => {
                    let first_page_is_cover = get_input_bool(&self.cover_toggle_ref)
                        .unwrap_or(data.first_page_is_cover);
                    let hide_sidebar = get_input_bool(&self.hide_toggle_ref)
                        .unwrap_or(data.hide_sidebar);
                    let show_help = get_input_bool(&self.show_help_ref)
                        .unwrap_or(data.show_help);
                    let show_magnifier = get_input_bool(&self.show_magnifier_ref)
                        .unwrap_or(data.show_magnifier);
                    let line_height = get_input_f64(&self.line_height_ref)
                        .unwrap_or(data.line_height);
                    let magnifier_height = get_input_u16(&self.magnifier_height_ref)
                        .unwrap_or(data.magnifier_height);
                    let magnifier_width = get_input_u16(&self.magnifier_width_ref)
                        .unwrap_or(data.magnifier_width);
                    let magnifier_radius = get_input_u8(&self.magnifier_radius_ref)
                        .unwrap_or(data.magnifier_radius);
                    let magnification = get_input_u16(&self.magnification_ref)
                        .unwrap_or(data.magnification);
                    let new_data = SidebarData {
                        first_page_is_cover,
                        hide_sidebar,
                        line_height,
                        magnifier_height,
                        magnifier_width,
                        magnifier_radius,
                        magnification,
                        show_help,
                        show_magnifier,
                    };
                    if new_data != *data {
                        commit.emit(new_data);
                    }
                    false
                }
            }
        }

        fn view(&self, ctx: &Context<Self>) -> Html {
            let Props { data, expanded, toggle_sidebar, .. } = ctx.props();
            let onblur = &self.onblur;
            if !(*expanded || data.hide_sidebar) {
                return html! {
                    <div id="SideBar" tabindex={"2"} onclick={toggle_sidebar.clone()} {onblur}>
                        {icons::burger()}
                    </div>
                };
            }

            let class = expanded.then_some("expanded");
            let hidden = data.hide_sidebar && !expanded;
            let onclick =
                if *expanded { Callback::noop() } else { toggle_sidebar.clone() };

            html! {
                <div id="SideBar" tabindex={"2"} {class} {hidden} {onclick} {onblur}>
                    <div class="sidebar-home-button-container">
                        <button onclick={toggle_sidebar}>{icons::chevron()}{"Close"}</button>
                        <Link<Route> to={Route::Home}>
                            <button>{icons::home()}{"Home"}</button>
                        </Link<Route>>
                    </div>
                    <hr/>
                    <h2>{"Volume Settings"}</h2>
                    <hr/>

                    <div class="sidebar-input-container">
                        <label for="first-page-cover">{"First Page Is Cover"}</label>
                        <input
                            ref={&self.cover_toggle_ref}
                            id="first-page-cover" type="checkbox"
                            checked={data.first_page_is_cover}
                            onchange={&self.onchange}
                        />
                    </div>

                    <div class="sidebar-input-container">
                        <label for="line-height">{"Line-Height"}</label>
                        <input
                            ref={&self.line_height_ref}
                            id="line-height" type="number"
                            min="0.5" max="2.5" step="0.05"
                            value={data.line_height.to_string()}
                            onchange={&self.onchange}
                        />
                    </div>

                    <h3 class="sidebar-header">{"Magnifier Settings"}</h3>
                    <div class="sidebar-input-container">
                        <label for="show-magnifier">{"Show Magnifier"}</label>
                        <input
                            ref={&self.show_magnifier_ref}
                            id="show-magnifier" type="checkbox"
                            checked={data.show_magnifier}
                            onchange={&self.onchange}
                        />
                    </div>
                    <div class="sidebar-input-container">
                        <label for="height">{"Height"}</label>
                        <input
                            ref={&self.magnifier_height_ref}
                            id="height" type="number"
                            min="100" max="1000" step="10"
                            value={data.magnifier_height.to_string()}
                            onchange={&self.onchange}
                        />
                    </div>
                    <div class="sidebar-input-container">
                        <label for="width">{"Width"}</label>
                        <input
                            ref={&self.magnifier_width_ref}
                            id="width" type="number"
                            min="100" max="1000" step="10"
                            value={data.magnifier_width.to_string()}
                            onchange={&self.onchange}
                        />
                    </div>
                    <div class="sidebar-input-container">
                        <label for="radius">{"Border Radius"}</label>
                        <input
                            ref={&self.magnifier_radius_ref}
                            id="radius" type="number"
                            min="0" max="100" step="5"
                            value={data.magnifier_radius.to_string()}
                            onchange={&self.onchange}
                        />
                    </div>
                    <div class="sidebar-input-container">
                        <label for="scale">{"Magnification"}</label>
                        <input
                            ref={&self.magnification_ref}
                            id="scale" type="number"
                            min="100" max="400" step="10"
                            value={data.magnification.to_string()}
                            onchange={&self.onchange}
                        />
                    </div>

                    <h3 class="sidebar-header">{"Misc"}</h3>
                    <div class="sidebar-input-container">
                        <label for="show-help">{"Show Help"}</label>
                        <input
                            ref={&self.show_help_ref}
                            id="show-help" type="checkbox"
                            checked={data.show_help}
                            onchange={&self.onchange}
                        />
                    </div>
                    <div class="sidebar-input-container">
                        <label for="hide-sidebar">{"Hide Sidebar"}</label>
                        <input
                            ref={&self.hide_toggle_ref}
                            id="hide-sidebar" type="checkbox"
                            checked={data.hide_sidebar}
                            onchange={&self.onchange}
                        />
                    </div>
                </div>
            }
        }
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
