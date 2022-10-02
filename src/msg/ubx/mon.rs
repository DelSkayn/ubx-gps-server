use serde::{Deserialize, Serialize};

use crate::{impl_struct, parse::ParseData};

impl_struct! {
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Msgpp{
    msg1:[u16; 8],
    msg2:[u16; 8],
    msg3:[u16; 8],
    msg4:[u16; 8],
    msg5:[u16; 8],
    skipped:[u16; 6],
}
}

impl_class! {
    pub enum Mon: PollMon{
        Msgpp(Msgpp)[120] = 0x06,
    }
}
