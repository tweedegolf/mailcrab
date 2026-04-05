use crate::{
    api::{fetch_message, fetch_raw},
    formatted::Formatted,
    overview::Tab,
    plaintext::Plaintext,
    types::{MailMessage, MailMessageMetadata},
};
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;
use yew::{
    Callback, Html, Properties, UseStateHandle, function_component, html, use_effect_with,
    use_state,
};

#[derive(Properties, PartialEq)]
pub struct ViewMessageProps {
    pub message: MailMessageMetadata,
    pub set_tab: Callback<Tab>,
    pub remove: Callback<MouseEvent>,
    pub active_tab: Tab,
}

#[function_component(ViewMessage)]
pub fn view(props: &ViewMessageProps) -> Html {
    let message: UseStateHandle<MailMessage> = use_state(Default::default);
    let raw_content: UseStateHandle<Option<String>> = use_state(|| None);

    // fetch message details
    let id = props.message.id.clone();
    let set_tab = props.set_tab.clone();
    let inner_message = message.clone();
    let current_tab = props.active_tab.clone();
    let raw_content_reset = raw_content.clone();

    use_effect_with(id, move |message_id| {
        let message_id = message_id.clone();
        raw_content_reset.set(None);
        spawn_local(async move {
            let message = fetch_message(&message_id).await;
            if message.html.is_empty() && current_tab == Tab::Formatted {
                set_tab.emit(Tab::Text)
            }
            if message.text.is_empty() && current_tab == Tab::Text {
                set_tab.emit(Tab::Formatted);
            }
            inner_message.set(message);
        });
        || ()
    });

    {
        let id = props.message.id.clone();
        let raw_content = raw_content.clone();
        let tab = props.active_tab.clone();

        use_effect_with((id, tab), move |(id, tab)| {
            if *tab == Tab::Raw {
                let id = id.clone();
                let raw_content = raw_content.clone();
                spawn_local(async move {
                    raw_content.set(Some(fetch_raw(&id).await));
                });
            }
            || ()
        });
    }

    if message.id.is_empty() {
        return html! {};
    }

    let mut tabs = vec![("Raw", Tab::Raw), ("Headers", Tab::Headers)];

    if !message.text.is_empty() && !message.html.is_empty() {
        tabs.push(("Plain", Tab::Text));
        tabs.push(("Formatted", Tab::Formatted));
    } else if !message.text.is_empty() {
        tabs.push(("Plain", Tab::Text));
    } else {
        tabs.push(("Formatted", Tab::Formatted));
    }

    let tabs: Vec<Html> = tabs
        .into_iter()
        .rev()
        .map(|(label, tab)| {
            let select_tab = tab.clone();
            let onclick = {
                let set_tab = props.set_tab.clone();
                move |_| set_tab.emit(select_tab.clone())
            };
            let class = if props.active_tab == tab {
                "active"
            } else {
                ""
            };

            html! {
              <li>
                <button
                  onclick={onclick}
                  class={class}
                >
                  {label}
                </button>
              </li>
            }
        })
        .collect();

    html! {
      <div class="view-inner">
        <ul class="tabs">
          {tabs}
          <li class="delete">
            <button onclick={props.remove.clone()}>
              {"Delete"}
            </button>
          </li>
        </ul>
        <div class="tab-content">
          if props.active_tab == Tab::Formatted {
            <Formatted message={(*message).clone()} />
          } else if props.active_tab == Tab::Text {
            <Plaintext message={(*message).clone()} />
          } else if props.active_tab == Tab::Headers {
            <table>
              <tbody>
                {message.headers.iter().map(|(key, value)| {
                  html! {
                    <tr>
                      <th>{key}</th>
                      <td>{value}</td>
                    </tr>
                  }
                }).collect::<Html>()}
              </tbody>
            </table>
          } else if props.active_tab == Tab::Raw {
            <pre>{(*raw_content).clone().unwrap_or_default()}</pre>
          }
        </div>
      </div>
    }
}
