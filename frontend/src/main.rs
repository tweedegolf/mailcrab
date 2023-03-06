use overview::Overview;

mod api;
mod formatted;
mod list;
mod message_header;
mod overview;
mod plaintext;
mod types;
mod view;
mod websocket;

fn main() {
    yew::Renderer::<Overview>::new().render();
}
