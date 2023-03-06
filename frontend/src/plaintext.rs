use crate::{message_header::MessageHeader, types::MailMessage};
use yew::{function_component, html, Html, Properties};

#[derive(Properties, Eq, PartialEq)]
pub struct PlaintextProps {
    pub message: MailMessage,
}

#[function_component(Plaintext)]
pub fn view(props: &PlaintextProps) -> Html {
    html! {
      <>
        <MessageHeader message={props.message.clone()} />
        <div class="body">
          <pre>{&props.message.text}</pre>
        </div>
      </>
    }
}
