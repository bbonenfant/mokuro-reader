use std::rc::Rc;

use rexie::Rexie;
use yew::{AttrValue, Callback, function_component, html, Html, HtmlResult};
use yew::suspense::{Suspense, use_future};
use yew_autoprops::autoprops;
use yew_router::{BrowserRouter, Routable, Switch};

use crate::errors::Result;
use crate::home::Home;
use crate::utils::db::{create_database, get_page};
use crate::utils::timestamp;

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
    // #[at("/secure")]
    // Secure,
    #[not_found]
    #[at("/404")]
    NotFound,
}

fn switch(db: &Rc<Rexie>, route: Route) -> Html {
    match route {
        Route::Home => html! { <Home {db}/> },
        // Route::Secure => html! {
        //     <Secure />
        // },
        Route::NotFound => html! { <h1>{ "404" }</h1> },
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
