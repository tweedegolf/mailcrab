use crate::{
    api::fetch_message,
    formatted::Formatted,
    overview::Tab,
    plaintext::Plaintext,
    types::{MailMessage, MailMessageMetadata},
};
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;
use yew::{
    function_component, html, use_effect_with_deps, use_state, Callback, Html, Properties,
    UseStateHandle,
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

    // fetch message details
    let id = props.message.id.clone();
    let set_tab = props.set_tab.clone();
    let inner_message = message.clone();
    use_effect_with_deps(
        |message_id| {
            let message_id = message_id.clone();
            spawn_local(async move {
                let message = fetch_message(&message_id).await;
                if message.html.is_empty() {
                    set_tab.emit(Tab::Text)
                } else {
                    set_tab.emit(Tab::Formatted)
                }
                inner_message.set(message);
            });
            || ()
        },
        id,
    );

    if message.id.is_empty() {
        return html! {};
    }

    let raw = base64::decode(&message.raw).unwrap();
    let raw = String::from_utf8_lossy(&raw);

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
            <pre>{&raw}</pre>
          }
        </div>
      </div>
    }
}
