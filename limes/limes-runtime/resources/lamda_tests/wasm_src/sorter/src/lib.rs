use limes_macro::limes_run;
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

#[limes_run]
fn run(args: String) -> String {
    let number_vec: Rc<RefCell<Vec<&str>>> = Rc::new(RefCell::new(Vec::new()));

    let split_vec_ref = Rc::clone(&number_vec);
    args.split(",").for_each(move |substring| {
        let mut local = split_vec_ref.borrow_mut();
        local.push(substring);
    });

    (*number_vec.borrow_mut()).sort();
    let vec_ref = number_vec.borrow();
    let mut result = String::from("[");
    for (index, elem) in vec_ref.iter().enumerate() {
        result.push_str(elem);
        if index != vec_ref.len() - 1 {
            result.push_str(",");
        } else {
            result.push_str("]");
        }
    }

    thread::sleep(Duration::from_secs(2));
    result
}
