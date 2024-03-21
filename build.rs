const MAIN_PATH: &str = "ui/app.slint";
const STYLE: &str = "fluent";

fn main() {
    let conf = slint_build::CompilerConfiguration::new().with_style(STYLE.into());
    slint_build::compile_with_config(MAIN_PATH, conf).unwrap_or_else(|e| {
        panic!(
            "{}: {e:?}; Failed to compile .slint file with given configuration!",
            core::any::type_name_of_val(&e),
        );
    });
}
