#![allow(dead_code)]

pub mod connection;
pub mod msg;
pub mod parse;

pub trait VecExt {
    fn shift(&mut self, by: usize);
}

impl<T: Copy> VecExt for Vec<T> {
    fn shift(&mut self, by: usize) {
        let len = self.len();
        assert!(len >= by);
        self.copy_within(by.., 0);
        self.truncate(len - by);
    }
}

pub fn deamonize() -> Result<(), ()> {
    let res = unsafe { libc::setsid() };
    match res {
        -1 => return Err(()),
        _ => {}
    }

    let res = unsafe { libc::fork() };
    match res {
        -1 => return Err(()),
        0 => return Ok(()),
        _ => std::process::exit(0),
    }
}
