use crate::memory::{self, MemoryHandle};

use anyhow::Result;
use log::{debug, trace};

const WINXP_BOARD_ADDRESS: u32 = 0x01005340;
const WINXP_BOARD_SIZE: usize = 0x360;

pub fn info(a_remote: MemoryHandle) -> Result<()> {
    debug!("Reading game board state");
    unsafe {
        let p_board = WINXP_BOARD_ADDRESS as *const _;
        let board: Vec<u8> = memory::copy_array(&a_remote, p_board, WINXP_BOARD_SIZE)?;
    }
    todo!()
}
