use std::rc::Rc;

use rexie::Rexie;
use wasm_bindgen::UnwrapThrowExt;
use yew::{html, Callback, Component, Context, Html};
use yew_router::{BrowserRouter, Routable, Switch};

use crate::errors::Result;
use crate::home::Home;
use crate::reader::Reader;
use crate::utils::db::create_database;

mod utils;
mod models;
mod errors;
mod upload;
mod home;
mod reader;
mod icons;

struct App {
    db: Option<Rc<Rexie>>,
}

enum Message {
    Set(Rc<Rexie>)
}

impl Component for App {
    type Message = Message;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self { db: None }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::Set(db) => {
                self.db = Some(db);
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
        if let Some(db) = &self.db {
            let db = db.clone();
            let render = Callback::from(
                move |route| switch(&db, route)
            );
            html! {
                <BrowserRouter>
                    <Switch<Route> {render} />
                </BrowserRouter>
            }
        } else { Html::default() }
    }
}

async fn initialize() -> Message {
    let db =
        Rc::new(create_database().await.expect_throw("failed to initialize database"));
    Message::Set(db)
}


#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    #[at("/volume/:volume_id/reader")]
    Reader { volume_id: u32 },
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(db: &Rc<Rexie>, route: Route) -> Html {
    match route {
        Route::Home => html! { <Home {db}/> },
        Route::Reader { volume_id } => html! { <Reader {db} {volume_id}/> },
        Route::NotFound => html! { <h1>{ "404" }</h1> },
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
