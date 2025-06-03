mod subscription;
mod utils;
mod window;

fn main() {
    iced::application(
        window::main::title,
        window::main::update,
        window::main::view,
    )
    .subscription(window::main::subscription)
    .run()
    .unwrap();
}
