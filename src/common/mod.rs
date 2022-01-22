use std::collections::VecDeque;

pub mod message;
pub mod metric;


pub fn vec_shift<T>(data : &mut VecDeque<T>, new_element : T, max_size : usize) {

    if data.len() < max_size {
        data.insert(0, new_element);
    } else {
        data.rotate_right(1);

        data[0] = new_element;
    }
}

