use crate::controlblock::ControlBlock;
use crate::buffer::ArrBuffer;
use std::sync::{Arc,Mutex};

pub struct Plotter<'a>{
    pub cb:&'a ControlBlock,
    pub buffers:Arc<Mutex<ArrBuffer<'a>>>,
}

impl <'a> Plotter<'a>{
    pub fn new(&self,cb:&'a ControlBlock,buffers:Arc<Mutex<ArrBuffer<'a>>>) -> Self {
        Plotter {
            cb,
            buffers,
        }
    }
    pub fn update_plot(&self,n_iter: i32) {
        if self.cb.plot_freq ==0{
            return
        }
        if self.buffers.lock().unwrap().t_id!=0{
            return
        }
        // let buffers=self.buffers.lock().unwrap();
        // let grid_m=buffers.grid_m;
        // let grid_n=buffers.grid_n;
        // let mut data=vec![vec![0.0;grid_n];grid_m];
        // for i in 0..grid_m {
        //     for j in 0..grid_n {
        //         data[i][j]=buffers.memory_pool[buffers.curr_offset+i*grid_n+j];
        //     }
        // }
        // println!("{:?}",data);

    }
}
