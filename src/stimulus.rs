use std::f64::consts::PI;
use std::sync::{Arc, Mutex};

use crate::buffer::ArrBuffer;

#[derive(Debug)]
pub struct Stimulus<'a> {
    buffers: Arc<Mutex<ArrBuffer<'a>>>,
    start_time: i32,
    duration: i32,
    tick: f64,
    row: usize,
    col: usize,
    amplitude: i32,
    period: i32,
}

impl<'a> Stimulus<'a> {
    pub fn new(
        buffers: Arc<Mutex<ArrBuffer<'a>>>,
        start_time: i32,
        duration: i32,
        row: usize,
        col: usize,
        period: i32,
    ) -> Self {
        Stimulus {
            buffers,
            start_time,
            duration,
            tick: 0.0,
            row,
            col,
            amplitude: 10,
            period,
        }
    }
    pub fn trigger_if_available(&mut self, iter: i32) -> bool {
        if iter > self.start_time + self.duration {
            return false;
        }
        if iter < self.start_time {
            return true;
        }
        if iter == self.start_time {
            self.tick = 0.0;
        }
        let mut buffers = self.buffers.lock().unwrap();
        if buffers.check_bounds(self.row, self.col) {
            let v: f64 =
                self.amplitude as f64 * (2.0 * PI * self.tick / (self.period as f64)).sin();
            let pair = buffers.map_to_local(self.row as i32, self.col as i32);

            if let Some(cv) = buffers.cur(pair.0, pair.1) {
                *cv = v;
            }
            if let Some(pv) = buffers.prev(pair.0, pair.1) {
                *pv = v;
            }
        }

        self.tick += 1.0;
        return true;
    }
}
