const ROUNDS: usize = 24;
const RC: [u64; ROUNDS] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

const R: [u32; 25] = [
    0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14,
];

/// Pure Rust Keccak-256 implementation for EIP-55 checksum calculation
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut state = [0u64; 25];
    let block_size = 136; // Rate for Keccak-256 (1088 bits / 8)

    // Pad input (Keccak-256 uses 0x01 byte padding, not SHA-3 0x06)
    let mut padded = data.to_vec();
    padded.push(0x01);
    while (padded.len() % block_size) != (block_size - 1) {
        padded.push(0x00);
    }
    padded.push(0x80);

    for chunk in padded.chunks(block_size) {
        for i in 0..(block_size / 8) {
            let word = u64::from_le_bytes(chunk[i * 8..(i + 1) * 8].try_into().unwrap());
            state[i] ^= word;
        }

        permutation(&mut state);
    }

    let mut out = [0u8; 32];
    for i in 0..4 {
        out[i * 8..(i + 1) * 8].copy_from_slice(&state[i].to_le_bytes());
    }
    out
}

#[inline]
fn permutation(state: &mut [u64; 25]) {
    for rc in RC.iter().take(ROUNDS) {
        theta(state);
        let b = rho_pi(state);
        chi(state, &b);
        state[0] ^= *rc;
    }
}

#[inline]
fn theta(state: &mut [u64; 25]) {
    // Theta
    let mut c = [0u64; 5];
    for x in 0..5 {
        c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
    }
    let mut d = [0u64; 5];
    for x in 0..5 {
        d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
    }
    for x in 0..5 {
        for y in 0..5 {
            state[x + y * 5] ^= d[x];
        }
    }
}

#[inline]
fn rho_pi(state: &[u64; 25]) -> [u64; 25] {
    // Rho and Pi
    let mut b = [0u64; 25];
    for x in 0..5 {
        for y in 0..5 {
            let idx = x + y * 5;
            let new_x = y;
            let new_y = (2 * x + 3 * y) % 5;
            b[new_x + 5 * new_y] = state[idx].rotate_left(R[idx]);
        }
    }

    b
}

#[inline]
fn chi(state: &mut [u64; 25], b: &[u64; 25]) {
    // Chi
    for x in 0..5 {
        for y in 0..5 {
            state[x + y * 5] =
                b[x + y * 5] ^ ((!b[((x + 1) % 5) + y * 5]) & b[((x + 2) % 5) + y * 5]);
        }
    }
}
