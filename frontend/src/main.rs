use overview::Overview;

mod api;
mod formatted;
mod list;
mod overview;
mod types;
mod view;
mod websocket;

fn main() {
    yew::start_app::<Overview>();
}
