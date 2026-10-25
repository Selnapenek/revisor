fn main() {
    // Icon of Revisor.exe in Explorer, the taskbar, and the Start menu.
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("../../brand/icon.ico")
        .compile()
        .expect("Cannot add the Windows icon");
}
