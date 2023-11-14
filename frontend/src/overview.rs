use futures::{channel::mpsc::Sender, StreamExt};
use gloo_console::error;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{
    api::fetch_messages_metadata,
    dark_mode::{init_dark_mode, toggle_dark_mode},
    list::MessageList,
    types::{Action, MailMessageMetadata},
    view::ViewMessage,
    websocket::WebsocketService,
};

pub enum Msg {
    Select(String),
    SetTab(Tab),
    Message(Box<MailMessageMetadata>),
    Messages(Vec<MailMessageMetadata>),
    Remove(String),
    Loading(bool),
    RemoveAll,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Tab {
    Formatted,
    Text,
    Headers,
    Raw,
}

pub struct Overview {
    selected: String,
    tab: Tab,
    messages: Vec<MailMessageMetadata>,
    sender: Sender<Action>,
    loading: bool,
}

impl Component for Overview {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link().clone();
        link.send_message(Msg::Loading(true));
        spawn_local(async move {
            let messages = fetch_messages_metadata().await;
            link.send_message(Msg::Messages(messages));
            link.send_message(Msg::Loading(false));
        });

        let mut wss = WebsocketService::new();

        let link = ctx.link().clone();
        spawn_local(async move {
            while let Some(message) = wss.receiver.next().await {
                link.send_message(Msg::Message(Box::new(message)));
            }
        });

        spawn_local(async {
            init_dark_mode();
        });

        Self {
            messages: vec![],
            tab: Tab::Formatted,
            selected: Default::default(),
            sender: wss.sender,
            loading: true,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Loading(value) => {
                self.loading = value;
            }
            Msg::Message(message) => {
                self.messages.push(*message);
            }
            Msg::Messages(messages) => {
                self.messages = messages;
            }
            Msg::Select(id) => {
                self.selected = id.clone();

                let unopened = self
                    .messages
                    .iter_mut()
                    .find(|m| m.id == self.selected && !m.opened);

                if let Some(unopened_message) = unopened {
                    if self.sender.try_send(Action::Open(id)).is_err() {
                        error!("Error registering email as opened");
                    }

                    unopened_message.opened = true;
                }
            }
            Msg::SetTab(tab) => {
                self.tab = tab;
            }
            Msg::Remove(id) => {
                self.messages.retain(|m| m.id != id);

                if self.sender.try_send(Action::Remove(id)).is_err() {
                    error!("Error removing email");
                }
            }
            Msg::RemoveAll => {
                if self.sender.try_send(Action::RemoveAll).is_ok() {
                    self.messages.clear();
                }
            }
        };

        true
    }

    fn rendered(&mut self, _ctx: &Context<Self>, _first_render: bool) {
        let count = self.messages.iter().filter(|m| !m.opened).count();
        gloo_utils::document().set_title(&format!("MailCrab ({})", count));
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let selected_message = self.messages.iter().find(|m| m.id == self.selected);
        let selected_id = self.selected.clone();

        html! {
          <>
            <header>
              <h1>{"Mail"}<span>{"Crab"}</span></h1>
              <div>
                if !self.messages.is_empty() {
                  <button onclick={link.callback(|_| Msg::RemoveAll)}>
                    {"Remove all"}<span>{"("}{self.messages.len()}{")"}</span>
                  </button>
                }
                <button class="dark-mode" title="Toggle dark mode" onclick={Callback::from(|_| {
                    toggle_dark_mode();
                })} />
              </div>
            </header>
            if self.messages.is_empty() {
              <div class="empty">
                if self.loading {
                    <div class="bouncing-loader">
                        <div></div>
                        <div></div>
                        <div></div>
                    </div>
                } else {
                    { "The inbox is empty 📭" }
                }
              </div>
            } else {
              <div class="main">
                <div class="list">
                    <ul>
                        <MessageList
                            messages={self.messages.clone()}
                            selected={self.selected.clone()}
                            select={link.callback(Msg::Select)}
                        />
                    </ul>
                </div>
                <div class="view">
                    if let Some(message) = selected_message {
                        <ViewMessage
                            message={message.clone()}
                            set_tab={link.callback(Msg::SetTab)}
                            remove={link.callback(move |_| Msg::Remove(selected_id.clone()))}
                            active_tab={self.tab.clone()}
                        />
                    }
                </div>
            </div>
            }
          </>
        }
    }
}
