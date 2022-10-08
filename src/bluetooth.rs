mod server;
pub use server::BluetoothServer;

mod client;
pub use client::BluetoothClient;

const SERVICE_UUID: uuid::Uuid = uuid::Uuid::from_u128(0xFEEDC0DE);
const CHARACTERISTIC_UUID: uuid::Uuid = uuid::Uuid::from_u128(0xFEEDC0DE00001);
const MANUFACTURER_ID: u16 = 0xf00d;
const PSM_LE_ADDR: u16 = bluer::l2cap::PSM_LE_DYN_START + 5;
