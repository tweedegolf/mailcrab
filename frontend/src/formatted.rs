use crate::types::MailMessage;
use yew::{function_component, html, Html, Properties};
use wasm_bindgen::{JsCast};
use web_sys::{Event, HtmlIFrameElement};

#[derive(Properties, Eq, PartialEq)]
pub struct FormattedProps {
    pub message: MailMessage,
}

fn try_set_font(e: Event) -> Option<()> {
  let target = e.target()?;
  let element = target.dyn_ref::<HtmlIFrameElement>()?;
      
  element
    .content_document()?
    .body()?
    .style()
    .set_css_text("font-family:sans-serif;line-height:1.5");

  Some(())
}

#[function_component(Formatted)]
pub fn view(props: &FormattedProps) -> Html {
    let message = &props.message;
    let body_src = format!("/api/message/{}/body", message.id);
    let onload = |e: Event| { try_set_font(e); };

    if message.id.is_empty() {
      return html! {};
    }

    html! {
      <>
        <table>
          <tbody>
            <tr>
              <th>{"From"}</th>
              <td>
                <span class="name">
                  {message.from.clone().name.unwrap_or_default()}
                </span>
                <span class="email">
                  {message.from.clone().email.unwrap_or_default()}
                </span>
              </td>
            </tr>
            <tr>
              <th>{"To"}</th>
              <td>
                {message.to.iter().map(|to| {
                  html! {
                    <span class="user">
                      <span class="name">
                        {to.clone().name.unwrap_or_default()}
                      </span>
                      <span class="email">
                        {to.clone().email.unwrap_or_default()}
                      </span>
                    </span>
                  }
                }).collect::<Html>()}
              </td>
            </tr>
            <tr>
              <th>{"Subject"}</th>
              <td>{&message.subject}</td>
            </tr>
          </tbody>
        </table>
        <div class="attachments">
          {message.attachments.iter().map(|a| {
            html! {
              <a
                href={format!("data:{};base64,{}", &a.mime, &a.content)}
                download={a.filename.clone()}
                class={&a.mime.replace('/', "-")}
              >
                {&a.filename}
                <span class="size">{&a.size}</span>
              </a>
            }
          }).collect::<Html>()}
        </div>
        <div class="body">
          <iframe onload={onload} src={body_src}></iframe>
        </div>
      </>
    }
}
