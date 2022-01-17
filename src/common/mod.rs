use std::collections::VecDeque;

pub mod message;
pub mod metric;


pub fn vec_shift<T>(data : &mut VecDeque<T>, new_element : T, max_size : usize) {

    if data.len() >= max_size {
        data.shrink_to(max_size - 1);
    }

    data.rotate_right(1);

    data.insert(0, new_element);
}

