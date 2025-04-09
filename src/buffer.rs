use crate::controlblock::ControlBlock;
use std::vec::Vec;

#[derive(Debug)]
pub struct ArrBuffer<'a> {
    pub cb: &'a ControlBlock, // 这里是引用，生命周期由 'a 指定
    pub t_id: i32,
    pub m: usize,
    pub n: usize,
    pub grid_m: usize,
    pub grid_n: usize,
    pub start_row: usize,
    pub start_col: usize,
    pub memory_pool: Vec<f64>,
    pub alpha: Vec<f64>,
    pub prev_offset: usize,
    pub curr_offset: usize,
    pub next_offset: usize,
}

impl<'a> ArrBuffer<'a> {
    pub fn new(cb: &'a ControlBlock, t_id: i32) -> Self {
        let (m, n, grid_m, grid_n);
        let mut start_rows = Vec::new();
        let mut start_cols = Vec::new();

        // 根据传入的 px 和 py 来确定 m, n, grid_m, grid_n
        if cb.px * cb.py == 1 {
            m = cb.m;
            n = cb.n;
            grid_m = cb.m + 2;
            grid_n = cb.n + 2;
        } else {
            n = cb.n / cb.px
                + if Self::get_extra_col(t_id, &cb) {
                    1
                } else {
                    0
                };
            m = cb.m / cb.py
                + if Self::get_extra_row(t_id, &cb) {
                    1
                } else {
                    0
                };
            grid_n = n + 2;
            grid_m = m + 2;
        }

        // 计算 startRows 和 startCols
        let mut start_c = 0;
        for col in 0..cb.px {
            start_cols.push(start_c);
            start_c += cb.n / cb.px
                + if Self::get_extra_col(col.try_into().unwrap(), &cb) {
                    1
                } else {
                    0
                };
        }

        let mut start_r = 0;
        for row in 0..cb.py {
            start_rows.push(start_r);
            start_r += cb.m / cb.py
                + if Self::get_extra_row(row.try_into().unwrap(), &cb) {
                    1
                } else {
                    0
                };
        }
        let start_row = start_rows[t_id as usize / cb.px as usize];
        let start_col = start_cols[t_id as usize % cb.px as usize];
        // 计算内存池的大小，并初始化为零
        let total_size = 3 * grid_m * grid_n;
        let memory_pool = vec![0.0; total_size];
        let alpha = vec![0.29*0.29; grid_m * grid_n];

        let prev_offset: usize = 0;
        let curr_offset: usize = grid_m * grid_n;
        let next_offset: usize = 2 * grid_m * grid_n;
        // 返回一个新的 ArrBuffers 实例
        Self {
            cb,
            t_id,
            m,
            n,
            grid_m,
            grid_n,
            start_row,
            start_col,
            memory_pool,
            alpha,
            prev_offset,
            curr_offset,
            next_offset,
        }
    }

    fn get_extra_col(t_id: i32, cb: &ControlBlock) -> bool {
        t_id as usize % cb.px < cb.n % cb.px
    }

    fn get_extra_row(t_id: i32, cb: &ControlBlock) -> bool {
        t_id as usize / cb.px < cb.m % cb.py
    }

    pub fn sum_sq(
        &self,
        r: usize,
        c: usize,
        rend: usize,
        cend: usize,
        grid: &Vec<Vec<f64>>,
    ) -> f64 {
        let mut sum_sq = 0.0;
        for i in r..rend {
            for j in c..cend {
                let v = grid[i][j];
                sum_sq += v * v;
            }
        }
        sum_sq
    }
    pub fn plot_buffer(&self){
        for r in 1..self.grid_m-1 {
            for c in 1..self.grid_n-1 {
                let v = self.prev_v(r, c);
                print!("{:.2} ", v);
            }
            println!();
        }
        println!("cur -------");

        for r in 1..self.grid_m-1 {
            for c in 1..self.grid_n-1 {
                let v = self.cur_v(r, c);
                print!("{:.2} ", v);
            }
            println!();
        }
        println!("next -------");

        for r in 1..self.grid_m-1 {
            for c in 1..self.grid_n-1 {
                let v = self.nxt_v(r, c);
                print!("{:.2} ", v);
            }
            println!();
        }

    }
    pub fn adv_buffers(&mut self) {
        // 交换 prev, curr, next 的引用
        let t = self.prev_offset;
        self.prev_offset = self.curr_offset;
        self.curr_offset = self.next_offset;
        self.next_offset = t;
    }

    pub fn check_bounds(&self, r: usize, c: usize) -> bool {
        let start_r = self.start_row;
        let start_c = self.start_col;
        r >= start_r && r < start_r + self.m && c >= start_c && c < start_c + self.n
    }
    pub fn map_to_local(&self, globr: i32, globc: i32) -> (usize, usize) {
        let start_r = self.start_row;
        let start_c = self.start_col;
        let local_r = globr - start_r as i32;
        let local_c = globc - start_c as i32;
        if local_r < 0 || local_r  >= self.m as i32 || local_c < 0 || local_c >= self.n as i32 {
            return (usize::MAX, usize::MAX);
        }
        (local_r as usize + 1 , local_c as usize + 1)
    }
    pub fn cur_v(&self, r: usize, c: usize) -> f64 {
        self.memory_pool[self.curr_offset + r * self.grid_n + c]
    }
    pub fn prev_v(&self, r: usize, c: usize) -> f64 {
        self.memory_pool[self.prev_offset + r * self.grid_n + c]
    }
    pub fn nxt_v(&self, r: usize, c: usize) -> f64 {
        self.memory_pool[self.next_offset + r * self.grid_n + c]
    }
    pub fn alp_v(&self, r: usize, c: usize) -> f64 {
        self.alpha[r * self.grid_n + c]
    }
    pub fn cur(&mut self, r: usize, c: usize) -> Option<&mut f64> {
        self.memory_pool.get_mut(self.curr_offset + r * self.grid_n + c)
    }
    pub fn prev(&mut self, r: usize, c: usize) -> Option<&mut f64> {
        self.memory_pool.get_mut(self.prev_offset + r * self.grid_n + c)
    }
    pub fn nxt(&mut self, r: usize, c: usize) -> Option<&mut f64> {
        self.memory_pool.get_mut(self.next_offset + r * self.grid_n + c)
    }

    pub fn extract_row(&self, r: usize) -> Vec<f64> {
        let start = self.curr_offset + r * self.grid_n;
        self.memory_pool[start..start + self.grid_n].to_vec()
    }
    pub fn update_row(&mut self, r: usize, values: &[f64]) {
        let start = self.curr_offset + r * self.grid_n;
        self.memory_pool[start..start + self.grid_n].copy_from_slice(values);
    }
    pub fn extract_col(&self, c: usize) -> Vec<f64> {
        let mut col = Vec::with_capacity(self.grid_m);
        for r in 0..self.grid_m {
            let idx = self.curr_offset + r * self.grid_n + c;
            col.push(self.memory_pool[idx]);
        }
        col
    }
    pub fn update_col(&mut self, c: usize, values: &[f64]) {
        for (r, &val) in values.iter().enumerate() {
            let idx = self.curr_offset + r * self.grid_n + c;
            self.memory_pool[idx] = val;
        }
    }
}
