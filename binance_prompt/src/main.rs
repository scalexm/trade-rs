#![feature(nll)]

extern crate trade_rs;
extern crate futures;
extern crate tokio;
extern crate cursive;
extern crate serde_json;
extern crate env_logger;
#[macro_use] extern crate failure;

mod prompt;

use std::fs::File;
use futures::sync::mpsc::UnboundedSender;
use std::cell::{Cell, RefCell};
use trade_rs::*;
use trade_rs::api::*;

use cursive::Cursive;
use cursive::theme::Theme;
use cursive::view::Position;
use cursive::views::*;
use cursive::traits::Boxable;

thread_local! {
    static PUSH: RefCell<Option<UnboundedSender<prompt::PushEvent>>> = RefCell::new(None);
    static TIME_WINDOW: Cell<u64> = Cell::new(1000);
}

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

fn draw_input(siv: &mut Cursive, price_tick: Tick, size_tick: Tick) {
    siv.add_layer(
        BoxView::with_full_width(
            EditView::new()
                .filler(" ")
                .on_submit(move |siv, content| {
                    submit_input(siv, content, price_tick, size_tick)
                })
        )
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
    PUSH.with(move |cell| {
        *cell.borrow_mut() = Some(push);
    });

    siv.add_layer(prompt.full_screen());

    draw_input(&mut siv, price_tick, size_tick);
    siv.run();
}

fn submit_input(siv: &mut Cursive, content: &str, price_tick: Tick, size_tick: Tick) {
    let args: Vec<_> = content.split(' ').collect();

    if args.is_empty() {
        return;
    }

    let cmd = &args[0];

    if *cmd == "quit" {
        siv.quit();
        return;
    }

    if let Err(err) = process_input(cmd, &args[1..], price_tick, size_tick) {
        PUSH.with(|cell| {
            let msg = format!("{}", err);
            cell.borrow()
                .as_ref()
                .unwrap()
                .unbounded_send(prompt::PushEvent::Message(msg))
                .unwrap();
        });
    }

    siv.pop_layer();
    draw_input(siv, price_tick, size_tick);
}

fn process_input(cmd: &str, args: &[&str], price_tick: Tick, size_tick: Tick)
    -> Result<(), Error>
{
    match cmd {
        "order" => {
            if args.len() < 4 {
                bail!("wrong number of arguments");
            }

            let side = match args[0].to_lowercase().as_ref() {
                "buy" => Side::Bid,
                "sell" => Side::Ask,
                other => bail!("expected side, got `{}`", other),
            };

            let size = size_tick.convert_unticked(&args[1])?;
            let price = price_tick.convert_unticked(&args[2])?;

            let time_in_force = match args[3].to_lowercase().as_ref() {
                "gtc" => TimeInForce::GoodTilCanceled,
                "ioc" => TimeInForce::ImmediateOrCancel,
                "fok" => TimeInForce::FillOrKilll,
                other => bail!("expected time in force, got `{}`", other),
            };

            let order = Order {
                side,
                size,
                price,
                time_in_force,
                order_id: None,
                time_window: TIME_WINDOW.with(|cell| cell.get()),
            };

            PUSH.with(move |cell| {
                cell.borrow()
                    .as_ref()
                    .unwrap()
                    .unbounded_send(prompt::PushEvent::Order(order))
                    .unwrap();
            });
        },
        "cancel" => {
            if args.len() < 1 {
                bail!("wrong number of arguments");
            }
            
            let cancel = Cancel {
                order_id: args[0].to_string(),
                time_window: TIME_WINDOW.with(|cell| cell.get()),
                cancel_id: None,
            };

            PUSH.with(move |cell| {
                cell.borrow()
                    .as_ref()
                    .unwrap()
                    .unbounded_send(prompt::PushEvent::Cancel(cancel))
                    .unwrap();
            });
        },
        "time_window" => {
            let tw = args[0].parse()?;
            if tw > 5000 {
                bail!("time window value too high: {}", tw);
            }

            TIME_WINDOW.with(|cell| cell.set(tw));
        },
        other => bail!("unknown command `{}`", other),
    }

    Ok(())
}
