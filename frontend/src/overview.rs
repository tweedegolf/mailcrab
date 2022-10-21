use futures::channel::mpsc::Sender;
use futures::StreamExt;
use gloo_console::error;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::api::fetch_messages_metadata;
use crate::list::MessageList;
use crate::types::{Action, MailMessageMetadata};
use crate::view::ViewMessage;
use crate::websocket::WebsocketService;

pub enum Msg {
    Select(String),
    SetTab(Tab),
    Message(MailMessageMetadata),
    Messages(Vec<MailMessageMetadata>),
    Remove(String),
    RemoveAll,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Tab {
    Formatted,
    Headers,
    Raw,
}

pub struct Overview {
    selected: String,
    tab: Tab,
    messages: Vec<MailMessageMetadata>,
    sender: Sender<Action>,
}

impl Component for Overview {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link().clone();
        spawn_local(async move {
            let messages = fetch_messages_metadata().await;
            link.send_message(Msg::Messages(messages));
        });

        let mut wss = WebsocketService::new();

        let link = ctx.link().clone();
        spawn_local(async move {
            while let Some(message) = wss.receiver.next().await {
                link.send_message(Msg::Message(message));
            }
        });

        Self {
            messages: vec![],
            tab: Tab::Formatted,
            selected: Default::default(),
            sender: wss.sender,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::Message(message) => {
                self.messages.push(message);
                true
            }
            Msg::Messages(messages) => {
                self.messages = messages;
                true
            }
            Msg::Select(id) => {
                self.selected = id.clone();

                let unopened = self
                    .messages
                    .iter_mut()
                    .find(|m| m.id == self.selected && !m.opened);

                if let Some(mut unopened_message) = unopened {
                    if self.sender.try_send(Action::Open(id)).is_err() {
                        error!("Error registering email as opened");
                    }
                    unopened_message.opened = true;
                }

                true
            }
            Msg::SetTab(tab) => {
                self.tab = tab;
                true
            }
            Msg::Remove(id) => {
                self.messages.retain(|m| m.id != id);

                if self.sender.try_send(Action::Remove(id)).is_err() {
                    error!("Error removing email");
                }

                true
            }
            Msg::RemoveAll => {
                if self.sender.try_send(Action::RemoveAll).is_ok() {
                    self.messages.clear();
                }
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();
        let selected_message = self.messages.iter().find(|m| m.id == self.selected);
        let selected_id = self.selected.clone();

        html! {
          <>
            <header>
              <h1>{"Mail"}<span>{"Crab"}</span></h1>
              if !self.messages.is_empty() {
                <button onclick={link.callback(|_| Msg::RemoveAll)}>
                  {"Remove all"}<span>{"("}{self.messages.len()}{")"}</span>
                </button>
              }
            </header>
            if self.messages.is_empty() {
              <div class="empty">
                {"The inbox is empty 📭"}
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
