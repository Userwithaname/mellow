use gtk::glib;

fn main() -> glib::ExitCode {
    mellow::init_globals();
    mellow::ui::Application::run()
}
