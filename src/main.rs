use futures::future::join_all;
use netcdf::{create, Extent, Extents};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::vec;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Barrier, RwLock};
use tokio::task;
use wave_2d::buffer::ArrBuffer;
use wave_2d::controlblock::ControlBlock;
use wave_2d::obstacle::clear_alpha_region;
use wave_2d::stimulus::Stimulus;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut tasks = vec![];
    let args = vec![
        "wave_2d",
        "-c",
        "tests/t500.config",
        "-i",
        "200",
        "-x",
        "1",
        "-y",
        "1",
    ];
    let args_string: Vec<String> = args.iter().map(|&s| String::from(s)).collect();
    let task_config: ControlBlock = ControlBlock::new(args_string);
    let grid_size: usize = task_config.m;
    let num_threads = task_config.px * task_config.py;
    let global_grid: Arc<RwLock<Vec<f64>>> =
        Arc::new(RwLock::new(vec![0.0; grid_size * grid_size]));
    let barrier = Arc::new(Barrier::new(num_threads + 1)); // +1 for the main thread
    let mut file = create("output.nc")?;
    file.add_dimension("y", grid_size)?;
    file.add_dimension("x", grid_size)?;
    file.add_unlimited_dimension("frame")?;
    let mut data_var = file.add_variable::<f64>("data", &["frame", "y", "x"])?; 
    let mut senders: Vec<Sender<Vec<f64>>> = vec![];
    let mut receivers: Vec<Receiver<Vec<f64>>> = vec![];

    for _ in 0..num_threads {
        let (tx, rx) = mpsc::channel(4); 
        senders.push(tx);
        receivers.push(rx);
    }
    for tid in 0..num_threads {
        let shared_grid: Arc<RwLock<Vec<f64>>> = Arc::clone(&global_grid);
        let barrier = Arc::clone(&barrier);
        let mut my_receiver: Receiver<Vec<f64>> = receivers.remove(0); 
        let my_senders = senders.clone(); 
        let cb = task_config.clone();
        let (top_t_id, bot_t_id, left_t_id, right_t_id) =
            compute_neighbors(tid as i32, cb.px as i32, cb.py as i32);
        let top_global_edge = tid < cb.px;
        let bot_global_edge = tid >= cb.px * (cb.py - 1);
        let left_global_edge = tid % cb.px == 0;
        let right_global_edge = (tid + 1) % cb.px == 0;
        let task = task::spawn(async move {
            let arr_buffers: Arc<Mutex<ArrBuffer<'_>>> =
                Arc::new(Mutex::new(ArrBuffer::new(&cb, tid as i32)));

            let mut s_list: Vec<Stimulus> = Vec::new();
            if cb.config.get("objects").is_some() {
                let objects = cb.config.get("objects").unwrap();
                for object in objects.as_array().unwrap() {
                    let obj_type = object.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match obj_type {
                        "sine" => {
                            let start_time =
                                object.get("start").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                            let duration =
                                object.get("duration").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                            let row =
                                object.get("row").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                            let col =
                                object.get("col").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                            let period =
                                object.get("period").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

                            let buffers = Arc::clone(&arr_buffers);
                            let s = Stimulus::new(buffers, start_time, duration, row, col, period);
                            s_list.push(s);
                        }

                        "rectobstacle" => {
                            let row =
                                object.get("row").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                            let col =
                                object.get("col").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                            let width =
                                object.get("width").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                            let height =
                                object.get("height").and_then(|v| v.as_i64()).unwrap_or(0) as usize;
                            clear_alpha_region(Arc::clone(&arr_buffers), row, col, width, height);
                        }

                        _ => {
                            eprintln!("Unknown object type: {:?}", obj_type);
                        }
                    }
                }
            }
            let start_row;
            let start_col;
            let grid_m;
            let grid_n;
            {
                let u_val: std::sync::MutexGuard<'_, ArrBuffer<'_>> = arr_buffers.lock().unwrap();
                start_row = u_val.start_row;
                start_col = u_val.start_col;
                grid_m = u_val.grid_m;
                grid_n = u_val.grid_n;
            }
        
            let mut iter = 0;
            while !s_list.is_empty() && iter < cb.niters {
                s_list.retain_mut(|it: &mut Stimulus<'_>| it.trigger_if_available(iter as i32));
                if cb.px * cb.py != 1 {
                    exchange_ghost_cells(
                        Arc::clone(&arr_buffers),
                        &mut my_receiver,
                        &my_senders,
                        top_t_id,
                        bot_t_id,
                        left_t_id,
                        right_t_id,
                    )
                    .await;
                }

                compute_u(Arc::clone(&arr_buffers));
                compute_edge_u(
                    Arc::clone(&arr_buffers),
                    top_global_edge,
                    bot_global_edge,
                    left_global_edge,
                    right_global_edge,
                );

                {
                    let mut grid = shared_grid.write().await;

                    let u_val: std::sync::MutexGuard<'_, ArrBuffer<'_>> =
                        arr_buffers.lock().unwrap();

                    for i in 0..grid_m - 2 {
                        for j in 0..grid_n - 2 {
                            grid[grid_size * (start_row + i) + start_col + j] =
                                u_val.cur_v(i + 1, j + 1);
                        }
                    }
                }
                barrier.wait().await;
                barrier.wait().await;
                {
                    let mut buffers = arr_buffers.lock().unwrap();
                    buffers.adv_buffers();
                }
                iter += 1;
            }

            while iter < cb.niters {
                {
                    if cb.px * cb.py != 1 {
                        exchange_ghost_cells(
                            Arc::clone(&arr_buffers),
                            &mut my_receiver,
                            &my_senders,
                            top_t_id,
                            bot_t_id,
                            left_t_id,
                            right_t_id,
                        )
                        .await;
                    }
                    compute_u(Arc::clone(&arr_buffers));
                    compute_edge_u(
                        Arc::clone(&arr_buffers),
                        top_global_edge,
                        bot_global_edge,
                        left_global_edge,
                        right_global_edge,
                    );
                    let mut grid = shared_grid.write().await;
                    let u_val: std::sync::MutexGuard<'_, ArrBuffer<'_>> =
                        arr_buffers.lock().unwrap();
                    for i in 0..grid_m - 2 {
                        for j in 0..grid_n - 2 {
                            grid[grid_size * (start_row + i) + start_col + j] =
                                u_val.cur_v(i + 1, j + 1);
                        }
                    }
                }
                barrier.wait().await;
                barrier.wait().await;
                {
                    let mut buffers = arr_buffers.lock().unwrap();
                    buffers.adv_buffers();
                }
                iter += 1;
            }
        });

        tasks.push(task);
    }
    let start_time = Instant::now();

    for frame_id in 0..task_config.niters {
        barrier.wait().await;
        let grid = global_grid.read().await;
        let extents: Extents = [
            Extent::SliceCount {
                start: frame_id,
                count: 1,
                stride: 1,
            },
            Extent::SliceCount {
                start: 0,
                count: grid_size,
                stride: 1,
            },
            Extent::SliceCount {
                start: 0,
                count: grid_size,
                stride: 1,
            },
        ]
        .into();
        data_var.put_values(&grid, extents)?;
        barrier.wait().await;
    }

    join_all(tasks).await;
    println!("Simulation finished! {:?}", start_time.elapsed());
    Ok(())
}

fn compute_u(buffers: Arc<Mutex<ArrBuffer>>) {
    let mut u = buffers.lock().unwrap();
    let grid_m = u.grid_m;
    let grid_n = u.grid_n;
    for r in 2..grid_m - 2 {
        for c in 2..grid_n - 2 {
            let nv = u.alp_v(r, c)
                * (u.cur_v(r - 1, c) + u.cur_v(r + 1, c) + u.cur_v(r, c - 1) + u.cur_v(r, c + 1)
                    - 4.0 * u.cur_v(r, c))
                + 2.0 * u.cur_v(r, c)
                - u.prev_v(r, c);
            if let Some(v) = u.nxt(r, c) {
                *v = nv;
            }
        }
    }
}

fn compute_edge_u(
    buffers: Arc<Mutex<ArrBuffer>>,
    top_global_edge: bool,
    bot_global_edge: bool,
    left_global_edge: bool,
    right_global_edge: bool,
) {
    let kappa = 0.2899999999999999;
    let mut u = buffers.lock().unwrap();
    let grid_m = u.grid_m;
    let grid_n = u.grid_n;
    for c in 1..grid_n - 1 {
        for &r in &[1, grid_m - 2] {
            let nv = u.alp_v(r, c)
                * (u.cur_v(r - 1, c) + u.cur_v(r + 1, c) + u.cur_v(r, c - 1) + u.cur_v(r, c + 1)
                    - 4.0 * u.cur_v(r, c))
                + 2.0 * u.cur_v(r, c)
                - u.prev_v(r, c);
            if let Some(v) = u.nxt(r, c) {
                *v = nv;
            }
        }
    }
    for r in 1..grid_m - 1 {
        for &c in &[1, grid_n - 2] {
            let nv = u.alp_v(r, c)
                * (u.cur_v(r - 1, c) + u.cur_v(r + 1, c) + u.cur_v(r, c - 1) + u.cur_v(r, c + 1)
                    - 4.0 * u.cur_v(r, c))
                + 2.0 * u.cur_v(r, c)
                - u.prev_v(r, c);
            if let Some(v) = u.nxt(r, c) {
                *v = nv;
            }
        }
    }
    if top_global_edge {
        let r = 0;
        for c in 1..grid_n - 1 {
            let nv = u.cur_v(r + 1, c)
                + ((kappa - 1.0) / (kappa + 1.0)) * (u.nxt_v(r + 1, c) - u.cur_v(r, c));
            if let Some(v) = u.nxt(r, c) {
                *v = nv;
            }
        }
    }
    if bot_global_edge {
        let r = grid_m - 1;
        for c in 1..grid_n - 1 {
            let nv = u.cur_v(r - 1, c)
                + ((kappa - 1.0) / (kappa + 1.0)) * (u.nxt_v(r - 1, c) - u.cur_v(r, c));
            if let Some(v) = u.nxt(r, c) {
                *v = nv;
            }
        }
    }
    if left_global_edge {
        let c = 0;
        for r in 1..grid_m - 1 {
            let nv = u.cur_v(r, c + 1)
                + ((kappa - 1.0) / (kappa + 1.0)) * (u.nxt_v(r, c + 1) - u.cur_v(r, c));
            if let Some(v) = u.nxt(r, c) {
                *v = nv;
            }
        }
    }
    if right_global_edge {
        let c = grid_n - 1;
        for r in 1..grid_m - 1 {
            let nv = u.cur_v(r, c - 1)
                + ((kappa - 1.0) / (kappa + 1.0)) * (u.nxt_v(r, c - 1) - u.cur_v(r, c));
            if let Some(v) = u.nxt(r, c) {
                *v = nv;
            }
        }
    }
}

fn compute_neighbors(t_id: i32, px: i32, py: i32) -> (i32, i32, i32, i32) {
    let x = t_id % px;
    let y = t_id / px;

    let top = if y > 0 { t_id - px } else { -1 };
    let bottom = if y < py - 1 { t_id + px } else { -1 };
    let left = if x > 0 { t_id - 1 } else { -1 };
    let right = if x < px as i32 - 1 { t_id + 1 } else { -1 };

    (top, bottom, left, right)
}

pub async fn exchange_ghost_cells<'a>(
    buffers: Arc<Mutex<ArrBuffer<'a>>>,
    my_receiver: &mut Receiver<Vec<f64>>,
    my_senders: &[Sender<Vec<f64>>],
    top_t_id: i32,
    bot_t_id: i32,
    left_t_id: i32,
    right_t_id: i32,
) {
    let (top_row, bot_row, left_col, right_col, grid_m, grid_n) = {
        let u = buffers.lock().unwrap();

        let top = if top_t_id >= 0 {
            Some(u.extract_row(1))
        } else {
            None
        };

        let bot = if bot_t_id >= 0 {
            Some(u.extract_row(u.grid_m - 2))
        } else {
            None
        };

        let left = if left_t_id >= 0 {
            Some(u.extract_col(1))
        } else {
            None
        };

        let right = if right_t_id >= 0 {
            Some(u.extract_col(u.grid_n - 2))
        } else {
            None
        };

        (top, bot, left, right, u.grid_m, u.grid_n)
    };
    if let Some(mut data) = top_row {
        data.push(1.0);
        let _ = my_senders[top_t_id as usize].send(data).await;
    }
    if let Some(mut data) = bot_row {
        data.push(2.0);
        let _ = my_senders[bot_t_id as usize].send(data).await;
    }
    if let Some(mut data) = left_col {
        data.push(3.0);
        let _ = my_senders[left_t_id as usize].send(data).await;
    }
    if let Some(mut data) = right_col {
        data.push(4.0);
        let _ = my_senders[right_t_id as usize].send(data).await;
    }

    let num_ghosts = [top_t_id, bot_t_id, left_t_id, right_t_id]
        .iter()
        .filter(|&&r| r >= 0)
        .count();
    let mut received = 0;
    while received < num_ghosts {
        if let Some(mut data) = my_receiver.recv().await {
            if let Some(dir_code) = data.pop() {
                let mut u = buffers.lock().unwrap();
                match dir_code as u8 {
                    1 => u.update_row(grid_m - 1, &data),
                    2 => u.update_row(0, &data),
                    3 => u.update_col(grid_n - 1, &data),
                    4 => u.update_col(0, &data),
                    _ => eprintln!("Unknown direction code: {}", dir_code),
                }
                received += 1;
            }
        }
    }
}
