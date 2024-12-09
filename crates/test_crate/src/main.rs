use ui_item::*;
use ui_item_derive::{UiDisplayable, UiSettable};

#[derive(UiDisplayable, UiSettable)]
struct Test {
    member1: f32,
    member2: u32,
}

fn main() {
    println!("Hello, world!");
}
