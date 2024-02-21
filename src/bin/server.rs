use core::panic;
use std::ffi::c_void;
use std::{mem::MaybeUninit, time::Instant};

use io_ruring_sys::*;

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

fn io_uring_for_each_cqe<F>(ring: &mut io_uring, mut handle_cqe: F)
where
    F: FnMut(&io_uring_cqe, &mut io_uring),
{
    let mut head = unsafe { *ring.cq.khead };
    //let tail = unsafe { io_uring_smp_load_acquire(ring.cq.ktail) };
    let tail = {
        // Create an atomic view of the allocated value
        let atomic = unsafe { std::sync::atomic::AtomicU32::from_ptr(ring.cq.ktail) };
        atomic.load(std::sync::atomic::Ordering::Acquire)
        // Use `atomic` for atomic operations, possibly share it with other threads
    };

    while head != tail {
        let index = io_uring_cqe_index(&ring, head, ring.cq.ring_mask);
        // Safety: Accessing the CQE array directly can be unsafe. Ensure that index calculations are correct.
        let cqe = unsafe { &mut *ring.cq.cqes.add(index as usize) };

        // Call the provided closure with the CQE.
        handle_cqe(cqe, ring);

        head = head.wrapping_add(1);
    }

    // Safety: Ensure that the head is updated correctly and visible to other threads.
    {
        // Create an atomic view of the allocated value
        let atomic = unsafe { std::sync::atomic::AtomicU32::from_ptr(ring.cq.khead) };
        atomic.store(head, std::sync::atomic::Ordering::Release);
    }
}
const OPERATIONS: usize = 100_000_000;
use std::os::fd::{AsRawFd, RawFd};
const NEW_CLIENT: u64 = 0xffffffffffffffff;

use std::alloc::{self, Layout};

pub struct Page {
    data: *mut u8,
    layout: Layout,
}

impl Page {
    pub fn new(size: usize) -> Self {
        // Create a layout for the given size with the alignment.
        let layout = Layout::from_size_align(size, 4096).expect("Failed to create layout");

        // Allocate memory using global allocator
        let data = unsafe { alloc::alloc(layout) };

        // Check for null pointer (allocation failure)
        if data.is_null() {
            alloc::handle_alloc_error(layout);
        }

        Self { data, layout }
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.data as *const u8
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data
    }

    pub fn len(&self) -> usize {
        self.layout.size()
    }
}

struct Client {
    fd: i32,
    page: Page,
}

fn main() {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:4444").expect("could not open listening socket");
    let fd = listener.as_raw_fd();
    println!("fd {}", fd);

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

        let mut clients = Vec::new();
        let mut sqe = io_uring_get_sqe(&mut ring);
        io_uring_prep_multishot_accept(sqe, fd, std::ptr::null_mut(), std::ptr::null_mut(), 0);
        io_uring_sqe_set_data64(sqe, NEW_CLIENT);
        println!("Server ready!");

        loop {
            //let mut sqe:  io_uring_sqe = io_uring_get_sq
            //let mut sqe = io_uring_get_sqe(&mut ring);
            //io_uring_prep_nop(sqe);
            //io_uring_sqe_set_data64(sqe, 1);

            io_uring_submit_and_wait(&mut ring, 1);

            io_uring_for_each_cqe(&mut ring, |cqe, ring| {
                if cqe.res < 0 {
                    panic!("result not as expected {}", -cqe.res);
                }
                let data = io_uring_cqe_get_data64(cqe);
                if data == NEW_CLIENT {
                    let cid = clients.len();
                    println!("Client id {}", cid);
                    let fd = cqe.res;
                    let client = Client {
                        fd,
                        page: Page::new(1024),
                    };
                    clients.push(client);
                    let sqe = io_uring_get_sqe(ring);
                    io_uring_prep_recv(sqe, fd, clients[cid].page.as_mut_ptr() as *mut c_void, 1024, 0x100); // MSG_WAITALL 
                        io_uring_sqe_set_data64(sqe, cid as u64);
                } else {
                    let cid = data; 
                    assert!((cid as usize) < clients.len());
                    let client = &mut clients[cid as usize];
                    if cqe.res == 0{
                        println!("client id {} disconnected", cid);
                    }

                    assert!(cqe.res == 1024);

                    //let sqe = io_uring_get_sqe(ring);
                    //io_uring_prep_send(sqe, client.fd, client.page.as_mut_ptr() as *mut c_void, 1024, 0x100); // MSG_WAITALL POSSIBLY TAKE NEW BUFFER HERE 

                    let sqe = io_uring_get_sqe(ring);
                    io_uring_prep_recv(sqe, client.fd, client.page.as_mut_ptr() as *mut c_void, 1024, 0x100); // MSG_WAITALL 
                    io_uring_sqe_set_data64(sqe, cid);
                }
            })
        }
    }
}
