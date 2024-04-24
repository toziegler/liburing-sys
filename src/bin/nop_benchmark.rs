use std::time::Instant;

use io_ruring_sys::*;

const OPERATIONS: usize = 100_000_000;
fn main() {
    unsafe {
        //MaybeUninit::<(u8, bool)>::zeroed()
        let mut ring: io_uring = std::mem::zeroed();
        let mut params: io_uring_params = std::mem::zeroed();
        params.flags = IORING_SETUP_SINGLE_ISSUER | IORING_SETUP_DEFER_TASKRUN;
        //params.flags = IORING_SETUP_SINGLE_ISSUER; //| IORING_SETUP_DEFER_TASKRUN;

        let ret = io_uring_queue_init_params(8, &mut ring, &mut params);
        if ret < 0 {
            panic!("could not initialize the ring");
        }

        let mut data_start: u64 = 0;
        let mut data_expect: u64 = 0;
        let start = Instant::now();
        for _ in 0..OPERATIONS {
            //let mut sqe:  io_uring_sqe = io_uring_get_sq
            let sqe = io_uring_get_sqe(&mut ring);
            io_uring_prep_nop(sqe);
            io_uring_sqe_set_data64(sqe, data_start);
            data_start += 1;

            io_uring_submit_and_wait(&mut ring, 1);

            let mut nr: u32 = 0;
            io_uring_for_each_cqe(&mut ring, |cqe, _| {
                let data = io_uring_cqe_get_data64(cqe);
                assert_eq!(data, data_expect);
                data_expect += 1;
                nr += 1;
            });
            io_uring_cq_advance(&mut ring, nr);
        }
        let elapsed = start.elapsed();
        println!("100e6 operations took {}", elapsed.as_secs_f64());
        println!(
            "Total ops: {} MOPs/sec",
            (OPERATIONS as f64 / elapsed.as_secs_f64())
        );
    }
}
