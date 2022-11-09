use anchor_lang::prelude::*;

#[repr(packed)]
#[account(zero_copy)]
pub struct VrfClient {
    pub bump: u8,
    pub max_result: u64,
    pub result_buffer: [u8; 32],
    pub result: u128,
    pub last_timestamp: i64,
    pub authority: Pubkey,
    pub vrf: Pubkey,
}
impl Default for VrfClient {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[event]
pub struct RequestingRandomness {
    pub vrf_client: Pubkey,
    pub max_result: u64,
    pub timestamp: i64,
}

#[event]
pub struct VrfClientInvoked {
    pub vrf_client: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct VrfClientResultUpdated {
    pub vrf_client: Pubkey,
    pub result: u128,
    pub result_buffer: [u8; 32],
    pub timestamp: i64,
}
