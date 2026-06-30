fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icons/slippy.ico");
        resource
            .compile()
            .expect("failed to compile Windows icon resource");
    }
}
