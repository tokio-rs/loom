#![deny(warnings, rust_2018_idioms)]

use loom::cell::Cell;

thread_local! {
    static ACTIVE_FRAME: Cell<()> = Cell::new(());
}

#[test]
#[should_panic = "Tried to access an object using a reference that belongs to a different store. This might indicate you are trying to reuse an object from an earlier execution"]
fn test_cell_reuse() {
    loom::model(|| {
        let handle_a = loom::thread::spawn(|| {
            let _ = ACTIVE_FRAME.with(Cell::get);
        });
        let handle_b = loom::thread::spawn(|| ());
        handle_a.join().unwrap();
        handle_b.join().unwrap();
    });
}
