use crate::types::MailMessageMetadata;
use js_sys::Date;
use timeago::Formatter;
use yew::{function_component, html, use_effect, use_state, Callback, Properties};
use yew_hooks::use_interval;

#[derive(Properties, PartialEq)]
pub struct MessageListProps {
    pub messages: Vec<MailMessageMetadata>,
    pub selected: String,
    pub select: Callback<String>,
}

fn get_now() -> u64 {
    (Date::now() / 1000.0) as u64
}

#[function_component(MessageList)]
pub fn list(props: &MessageListProps) -> Html {
    let formatter = Formatter::new();
    let now = use_state(get_now);

    {
        let now = now.clone();
        use_interval(
            move || {
                now.set(get_now());
            },
            10 * 1000,
        );
    }

    {
        let count = props.messages.iter().filter(|m| !m.opened).count();
        use_effect(move || {
            gloo_utils::document().set_title(&format!("MailCrab ({})", count));

            || ()
        });
    }

    props
        .messages
        .iter()
        .map(|message| {
            let id = message.id.clone();
            let select = props.select.clone();
            let onclick = { Callback::from(move |_| select.emit(id.clone())) };

            let class = if props.selected == message.id {
                "selected"
            } else if message.opened {
                "opened"
            } else {
                ""
            };

            let ago = if message.time > *now {
                std::time::Duration::from_secs(0)
            } else {
                std::time::Duration::from_secs(*now - message.time)
            };

            html! {
              <li
                tabIndex="0"
                onclick={onclick}
                class={class}
              >
                <span class="head">
                  <span class="from">
                    <span class="name">
                      {message.from.clone().name.unwrap_or_default()}
                    </span>
                    <span class="email">
                      {&message.from.clone().email.unwrap_or_default()}
                    </span>
                  </span>
                  <span class="date" title={message.date.clone()}>
                    {formatter.convert(ago)}
                  </span>
                </span>
                <span class="preview">
                  <span class="subject">
                    {&message.subject}
                  </span>
                  <span class="size">
                    {&message.size}
                  </span>
                </span>
              </li>
            }
        })
        .collect()
}
