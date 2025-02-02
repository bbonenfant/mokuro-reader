use std::rc::Rc;

use rexie::Rexie;
use wasm_bindgen::UnwrapThrowExt;
use yew::{html, html::Scope, Callback, Component, Context, Html};
use yew_router::{BrowserRouter, Routable, Switch};

use crate::errors::Result;
use crate::home::Home;
use crate::models::VolumeId;
use crate::notify::{Notification, NotificationProvider};
use crate::reader::Reader;
use crate::utils::db::create_database;

mod utils;
mod models;
mod errors;
mod upload;
mod home;
mod reader;
mod icons;
mod notify;

struct App {
    db: Option<Rc<Rexie>>,
    notify: Option<Scope<NotificationProvider>>,
}

enum Message {
    SetDB(Rc<Rexie>),
    SetScope(Scope<NotificationProvider>),
}

impl Component for App {
    type Message = Message;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self { db: None, notify: None }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::SetDB(db) => {
                self.db = Some(db);
                true
            }
            Message::SetScope(scope) => {
                self.notify = Some(scope);
                true
            }
        }
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {
        if first_render {
            ctx.link().send_future(initialize());
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        match (&self.db, &self.notify) {
            (Some(db), Some(scope)) => {
                let db = db.clone();
                let scope = scope.clone();
                let render = Callback::from(move |route| {
                    let notify = scope.callback(notify::Message::Notify);
                    switch(&db, route, notify)
                });
                html! {
                    <BrowserRouter>
                        <Switch<Route> {render}/>
                    </BrowserRouter>
                }
            }
            _ => { Html::default() }
        }
    }
}

async fn initialize() -> Message {
    let db =
        Rc::new(create_database().await.expect_throw("failed to initialize database"));
    Message::SetDB(db)
}


#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    #[at("/volume/:volume_id/reader")]
    Reader { volume_id: VolumeId },
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(db: &Rc<Rexie>, route: Route, notify: Callback<Notification>) -> Html {
    match route {
        Route::Home => html! { <Home {db} {notify}/> },
        Route::Reader { volume_id } => html! { <Reader {db} {notify} {volume_id} /> },
        Route::NotFound => html! { <h1>{ "404" }</h1> },
    }
}

fn main() {
    let app =
        yew::Renderer::<App>::with_root(utils::web::query_selector("#App")).render();
    let notifier =
        yew::Renderer::<NotificationProvider>::with_root(utils::web::query_selector("#NotificationContainer")).render();
    app.send_message(Message::SetScope(notifier.clone()));
}
