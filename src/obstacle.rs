use crate::buffer::ArrBuffer;
use std::sync::{Arc, Mutex};

pub fn clear_alpha_region<'a>(
    buffer: Arc<Mutex<ArrBuffer<'a>>>,
    row: usize,
    col: usize,
    width: usize,
    height: usize,
) {
    let mut grid = buffer.lock().unwrap();
    let grid_n = grid.grid_n;
    for r in row..row + height {
        for c in col..col + width {
            if grid.check_bounds(r, c) {
                let pair = grid.map_to_local(r as i32, c as i32);
                grid.alpha[pair.0 * grid_n + pair.1] = 0.0;
            }
        }
    }
}

