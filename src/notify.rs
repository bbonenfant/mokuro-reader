use gloo_console as console;
use gloo_timers::callback::Timeout;
use std::collections::HashMap;
use yew::{html, Callback, Component, Context, Html};

type ID = u64;


#[derive(Clone, Eq, PartialEq)]
pub enum Notification {
    Warning(&'static str, String),
}

impl Notification {
    pub fn content(&self) -> &'static str {
        match self {
            Self::Warning(content, _) => content,
        }
    }
    #[allow(unused)]
    pub fn error(&self) -> &String {
        match self {
            Self::Warning(_, error) => error,
        }
    }
    pub fn log(&self) {
        match self {
            Self::Warning(_, error) => console::warn!(error),
        }
    }
}

pub enum Message {
    Notify(Notification),
    CancelTimer(ID),
    Delete(ID),
}

pub struct NotificationProvider {
    notifications: HashMap<ID, Notification>,
    timeouts: HashMap<ID, Timeout>,
}

impl Component for NotificationProvider {
    type Message = Message;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self { notifications: HashMap::new(), timeouts: HashMap::new() }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Message::Notify(notification) => {
                let id = crate::utils::timestamp();
                let timeout = {
                    let link = ctx.link().clone();
                    Timeout::new(5000, move || link.send_message(Message::Delete(id)))
                };
                notification.log();
                self.notifications.insert(id, notification);
                self.timeouts.insert(id, timeout);
                true
            }
            Message::CancelTimer(id) => {
                self.timeouts.remove(&id).map(drop);
                true
            }
            Message::Delete(id) => {
                self.notifications.remove(&id);
                self.timeouts.remove(&id).map(drop);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.notifications.is_empty() { return Html::default(); }
        let notifications: Vec<Html> = self.notifications.iter().map(
            |(&id, notification)| {
                let delete = ctx.link().callback(move |_| Message::Delete(id));
                let onmouseenter = if self.timeouts.contains_key(&id) {
                    ctx.link().callback(move |_| Message::CancelTimer(id))
                } else { Callback::noop() };
                html! {
                    <div class="warning" {onmouseenter}>
                        <div class="warning-top">
                            <span>{"Unexpected Error"}</span>
                            <div class="warning-close" onclick={delete}>
                                {crate::icons::close()}
                            </div>
                        </div>
                        <p class="warning-content">{notification.content()}</p>
                    </div> }
            }
        ).collect();
        html! { <>{notifications}</> }
    }
}