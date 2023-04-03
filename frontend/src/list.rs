use crate::types::MailMessageMetadata;
use js_sys::Date;
use timeago::Formatter;
use yew::{
    function_component, html, html_nested, use_effect, use_state, Callback, Html, Properties,
};
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

            let mut classes = vec![];

            if props.selected == message.id {
                classes.push("selected");
            } else if message.opened {
                classes.push("opened")
            }

            if !message.attachments.is_empty() {
                classes.push("attachments");
            }

            let ago = if message.time > *now {
                std::time::Duration::from_secs(0)
            } else {
                std::time::Duration::from_secs(*now - message.time)
            };

            html! {
              <li
                tabIndex="0"
                onclick={onclick}
                class={classes.join(" ")}
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
                  <span class="recipients">
                    <span class="label">
                      if message.envelope_recipients.len() > 1 {
                        {"Recipients: "}
                      } else {
                        {"Recipient: "}
                      }
                    </span>
                    {for message.envelope_recipients.clone().into_iter().take(2).map(|addr| html_nested! {
                      <span class="email">{addr}</span>
                    })}
                    if message.envelope_recipients.len() > 2 {
                      <span class="etc">{", \u{2026}"}</span>
                    }
                  </span>
              </li>
            }
        })
        .collect()
}
