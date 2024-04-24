#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// Translating `io_uring_cqe_shift` to a Rust function
fn io_uring_cqe_shift(ring: &io_uring) -> u32 {
    if (ring.flags & IORING_SETUP_CQE32) != 0 {
        1
    } else {
        0
    }
}

// Translating `io_uring_cqe_index` to a Rust function
fn io_uring_cqe_index(ring: &io_uring, ptr: u32, mask: u32) -> u32 {
    (ptr & mask) << io_uring_cqe_shift(ring)
}

pub fn io_uring_for_each_cqe<F>(ring: &mut io_uring, mut handle_cqe: F)
where
    F: FnMut(&io_uring_cqe, &mut io_uring),
{
    let mut head = unsafe { *ring.cq.khead };
    let tail = {
        let atomic = unsafe { std::sync::atomic::AtomicU32::from_ptr(ring.cq.ktail) };
        atomic.load(std::sync::atomic::Ordering::Acquire)
    };

    while head != tail {
        let index = io_uring_cqe_index(ring, head, ring.cq.ring_mask);
        // Safety: Accessing the CQE array directly can be unsafe. Ensure that index calculations are correct.
        let cqe = unsafe { &mut *ring.cq.cqes.add(index as usize) };

        handle_cqe(cqe, ring);
        head = head.wrapping_add(1);
    }
}

//pub fn io_uring_cq_advance(ring: &mut io_uring, nr: u32) {
//    if nr > 0 {
//        let cq = &ring.cq;
//        let mut head = unsafe { *cq.khead };
//
//        let atomic = unsafe { std::sync::atomic::AtomicU32::from_ptr(ring.cq.khead) };
//        atomic.store(head + nr, std::sync::atomic::Ordering::Release);
//    }
//}

#[cfg(test)]
mod tests {
    use core::panic;

    use super::*;

    #[test]
    fn liburing_example() {
        unsafe {
            let mut ring: io_uring = std::mem::zeroed();
            let mut params: io_uring_params = std::mem::zeroed();
            params.flags = IORING_SETUP_SINGLE_ISSUER | IORING_SETUP_DEFER_TASKRUN;

            let ret = io_uring_queue_init_params(8, &mut ring, &mut params);
            if ret < 0 {
                panic!("could not initialize the ring");
            }

            let mut data_start: u64 = 0;
            let mut data_expect: u64 = 0;
            //loop {
            for _ in 0..20 {
                let sqe = io_uring_get_sqe(&mut ring);
                io_uring_prep_nop(sqe);
                io_uring_sqe_set_data64(sqe, data_start);
                data_start += 1;

                io_uring_submit_and_wait(&mut ring, 1);

                let mut nr: u32 = 0;
                io_uring_for_each_cqe(&mut ring, |cqe, _ring| {
                    let data = io_uring_cqe_get_data64(cqe);
                    assert_eq!(data, data_expect);
                    data_expect += 1;
                    nr += 1;
                });
                io_uring_cq_advance(&mut ring, nr);
            }
        }
    }
}
