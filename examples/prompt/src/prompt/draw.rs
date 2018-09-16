use trade::order_book;
use crate::prompt::Prompt;

use cursive::Printer;
use cursive::view::View;
use cursive::vec::Vec2;

impl View for Prompt {
    fn layout(&mut self, _: Vec2) {
        self.update();
    }

    fn draw(&self, printer: &Printer) {
        let order_book = format!("{}", self.order_book);
        for (i, line) in order_book.split('\n').enumerate() {
            printer.print((0, i), line);
        }

        printer.print((0, printer.size.y - 1), &self.output);

        for (i, order) in self.orders.values().enumerate() {
            let line = format!(
                "{}: {} @ {} ({:?})",
                order.order_id,
                order_book::display::displayable_size(order.size),
                order_book::display::displayable_price(order.price),
                order.side
            );
            printer.print((printer.size.x - line.len(), i), &line);
        }
    }
}
