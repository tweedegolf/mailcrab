use crate::{api::get_api_path, types::MailMessage};
use wasm_bindgen::JsCast;
use web_sys::{Event, HtmlIFrameElement, HtmlLinkElement};
use yew::{function_component, html, Html, Properties};

#[derive(Properties, Eq, PartialEq)]
pub struct FormattedProps {
    pub message: MailMessage,
}

fn try_set_font(e: &Event) -> Option<()> {
    let target = e.target()?;
    let element = target.dyn_ref::<HtmlIFrameElement>()?;

    element
        .content_document()?
        .body()?
        .style()
        .set_css_text("font-family:sans-serif;line-height:1.5");

    Some(())
}

fn try_set_link_targets(e: &Event) -> Option<()> {
    let target = e.target()?;
    let element = target.dyn_ref::<HtmlIFrameElement>()?;

    let links = element.content_document()?.query_selector_all("a").ok()?;

    for i in 0..links.length() {
        if let Some(l) = links.get(i) {
            l.unchecked_into::<HtmlLinkElement>().set_target("_blank")
        }
    }

    Some(())
}

#[function_component(Formatted)]
pub fn view(props: &FormattedProps) -> Html {
    let message = &props.message;
    let mut body_src = get_api_path("message/");
    body_src.push_str(message.id.as_str());
    body_src.push_str("/body");

    let onload = |e: Event| {
        try_set_font(&e);
        try_set_link_targets(&e);
    };

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
