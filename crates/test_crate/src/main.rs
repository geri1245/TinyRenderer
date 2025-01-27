use std::path::PathBuf;

use ui_item_derive::{UiDisplayable, UiSettableNew};

#[derive(UiDisplayable, UiSettableNew, Default)]
struct Embedded {
    #[ui_param(fmax = 25.0)]
    member1: u32,
    #[ui_param(valid_file_extensions = "jpg,png")]
    path: PathBuf,
}

#[derive(UiDisplayable, UiSettableNew, Default)]
struct Test {
    #[ui_param(fmin = 12.0, fmax = 25.0)]
    member1: f32,
    #[ui_param(min = 12, max = 25)]
    member2: i32,

    embedded: Embedded,
}

#[derive(UiDisplayable)]
enum Alma {
    Variant1(Test),
    Variant2,
}

fn main() {
    println!("Hello, world!");
}
