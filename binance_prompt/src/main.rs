#![feature(nll)]

extern crate trade_rs;
extern crate futures;
extern crate tokio;
extern crate cursive;
extern crate serde_json;
extern crate env_logger;
#[macro_use] extern crate failure;

mod prompt;
mod input;

use std::fs::File;
use trade_rs::{order_book, Tick};
use trade_rs::api::binance;

use cursive::Cursive;
use cursive::theme::Theme;
use cursive::view::Position;
use cursive::views::{EditView, LayerPosition};
use cursive::traits::Boxable;

fn build_siv() -> Cursive {
    let mut siv = Cursive::default();
    let theme = siv.current_theme();
    siv.set_theme(Theme {
        shadow: false,
        ..*theme
    });
    siv.set_fps(100);

    siv
}

fn draw_input_line(siv: &mut Cursive, price_tick: Tick, size_tick: Tick) {
    siv.add_layer(
        EditView::new()
            .filler(" ")
            .on_submit(move |siv, content| {
                input::submit_input(siv, content, price_tick, size_tick);
                siv.pop_layer();
                draw_input_line(siv, price_tick, size_tick);
            }).full_width()
    );
    siv.reposition_layer(
        LayerPosition::FromFront(0),
        Position::absolute((0, siv.screen_size().y - 4))
    );
}

fn main() {
    env_logger::init();

    let params = File::open("params.json").expect("cannot open `params.json`");
    let keys = File::open("keys.json").expect("cannot open `keys.json`");

    let params: binance::Params = serde_json::from_reader(params)
        .expect("expected valid JSON for `binance::Params`");
    let keys = serde_json::from_reader(keys)
        .expect("expected valid JSON for `binance::KeyPair`");

    let price_tick = params.symbol.price_tick;
    let size_tick = params.symbol.size_tick;
    order_book::display_price_tick(Some(price_tick));
    order_book::display_size_tick(Some(size_tick));

    let client = binance::Client::new(
        params,
        Some(keys)
    ).expect("unable to retrieve listen key");

    let mut siv = build_siv();

    let (prompt, push) = prompt::Prompt::new(client);
    input::push(push);

    siv.add_layer(prompt.full_screen());

    draw_input_line(&mut siv, price_tick, size_tick);
    siv.run();
}


