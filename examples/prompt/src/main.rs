#![feature(nll)]

#[macro_use] extern crate failure;

mod prompt;
mod input;

use std::fs::File;
use trade::api::{self, ApiClient, binance, gdax};
use clap::clap_app;

use cursive::Cursive;
use cursive::theme::Theme;
use cursive::view::Position;
use cursive::views::{EditView, LayerPosition};
use cursive::traits::Boxable;

fn build_siv() -> Cursive {
    let mut siv = Cursive::default();
    let theme = siv.current_theme().clone();
    siv.set_theme(Theme {
        shadow: false,
        ..theme
    });
    siv.set_fps(100);

    siv
}

fn draw_input_line<C: ApiClient>(siv: &mut Cursive) {
    siv.add_layer(
        EditView::new()
            .filler(" ")
            .on_submit(move |siv, content| {
                input::submit_input::<C>(siv, content);
                siv.pop_layer();
                draw_input_line::<C>(siv);
            }).full_width()
    );
    siv.reposition_layer(
        LayerPosition::FromFront(0),
        Position::absolute((0, siv.screen_size().y - 4))
    );
}

fn run<C: ApiClient + Send + 'static>(client: C, symbol: &str) {
    let mut siv = build_siv();

    let (prompt, push) = prompt::Prompt::new(client, symbol);
    input::push(push);

    siv.add_layer(prompt.full_screen());

    draw_input_line::<C>(&mut siv);
    siv.run();
}

fn main() {
    env_logger::init();

    let matches = clap_app!(api_prompt =>
        (version: "0.1.0")
        (author: "scalexm <martin.alex32@hotmail.fr>")
        (about: "Small CLI app for testing `trade-rs` API")
        (@arg exchange: +required "Exchange name (`gdax` or `binance`)")
        (@arg symbol: +required "Symbol name")
        (@arg params: -p --params +takes_value "Params file (default = `params.json`)")
        (@arg keys: -k --keys +takes_value "Keys file (default = `keys.json`)")
    ).get_matches();

    let params = matches.value_of("params").unwrap_or("params.json");
    let keys = matches.value_of("keys").unwrap_or("keys.json");

    let params = File::open(params).expect("cannot open params file");
    let keys = File::open(keys).expect("cannot open keys file");

    let params: api::Params = serde_json::from_reader(params)
        .expect("expected valid JSON for `api::Params`");

    let symbol = matches.value_of("symbol").unwrap();
    match matches.value_of("exchange").unwrap() {
        "binance" => {
            let keys = serde_json::from_reader(keys)
                .expect("expected valid JSON for `binance::KeyPair`");

            let client = binance::Client::new(
                params,
                Some(keys)
            ).expect("unable to create client");
            run(client, symbol);
        },

        "gdax" => {
            let keys = serde_json::from_reader(keys)
                .expect("expected valid JSON for `gdax::KeyPair`");

            let client = gdax::Client::new(
                params,
                Some(keys)
            ).expect("unable to create client");
            run(client, symbol);
        }

        other => {
            eprintln!("unsupported exchange: `{}`", other);
        }
    }
}
