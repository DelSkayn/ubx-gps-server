use crate::{impl_struct, parse::ParseData};

use serde::{Deserialize, Serialize};

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AckData{
    cls_id: u8,
    msg_id: u8,
}
}

impl_class! {
    pub enum Ack: PollAck{
        Ack(AckData)[2u16] = 0x01u8,
        Nak(AckData)[2u16] = 0x00u8,
    }
}
