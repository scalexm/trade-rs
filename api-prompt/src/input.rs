use trade::{Error, Tick, Side};
use trade::api::{Order, Cancel, TimeInForce};
use futures::sync::mpsc::UnboundedSender;
use std::cell::{RefCell, Cell};
use prompt::PushEvent;
use cursive::Cursive;

thread_local! {
    static PUSH: RefCell<Option<UnboundedSender<PushEvent>>> = RefCell::new(None);
    static TIME_WINDOW: Cell<u64> = Cell::new(1000);
}

pub fn push(push: UnboundedSender<PushEvent>) {
    PUSH.with(move |cell| {
        *cell.borrow_mut() = Some(push);
    });
}

pub fn submit_input(siv: &mut Cursive, content: &str, price_tick: Tick, size_tick: Tick) {
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
                .unbounded_send(PushEvent::Message(msg))
                .unwrap();
        });
    }
}

fn process_input(cmd: &str, args: &[&str], price_tick: Tick, size_tick: Tick)
    -> Result<(), Error>
{
    match cmd {
        side @ "buy" | side @ "sell" => {
            if args.len() < 3 {
                bail!("wrong number of arguments");
            }

            let side = match side {
                "buy" => Side::Bid,
                "sell" => Side::Ask,
                _ => unreachable!(),
            };

            let size = size_tick.convert_unticked(&args[0])?;
            let price = price_tick.convert_unticked(&args[1])?;

            let time_in_force = match args[2].to_lowercase().as_ref() {
                "gtc" => TimeInForce::GoodTilCanceled,
                "ioc" => TimeInForce::ImmediateOrCancel,
                "fok" => TimeInForce::FillOrKilll,
                other => bail!("expected time in force, got `{}`", other),
            };

            let mut order_id = None;
            if args.len() == 4 {
                order_id = Some(args[3].to_owned());
            }

            let order = Order {
                side,
                size,
                price,
                time_in_force,
                order_id,
                time_window: TIME_WINDOW.with(|cell| cell.get()),
            };

            PUSH.with(move |cell| {
                cell.borrow()
                    .as_ref()
                    .unwrap()
                    .unbounded_send(PushEvent::Order(order))
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
            };

            PUSH.with(move |cell| {
                cell.borrow()
                    .as_ref()
                    .unwrap()
                    .unbounded_send(PushEvent::Cancel(cancel))
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