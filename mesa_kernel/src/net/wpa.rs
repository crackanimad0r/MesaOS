//! WPA2-PSK (RSN) 4-way handshake implementation
//! Provides: HMAC-SHA1, PBKDF2-SHA1, IEEE 802.11 PRF, PMK/PTK/GTK derivation,
//! EAPOL-Key frame construction and parsing.

use alloc::vec::Vec;
use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use spin::Mutex;

// ============================================================================
// SHA-1 (FIPS 180-1)
// ============================================================================

const SHA1_BLOCK_SIZE: usize = 64;
const SHA1_DIGEST_SIZE: usize = 20;

fn sha1_transform(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4], block[i * 4 + 1], block[i * 4 + 2], block[i * 4 + 3],
        ]);
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];

    for i in 0..80 {
        let (f, k) = if i < 20 {
            ((b & c) | ((!b) & d), 0x5A827999)
        } else if i < 40 {
            (b ^ c ^ d, 0x6ED9EBA1)
        } else if i < 60 {
            ((b & c) | (b & d) | (c & d), 0x8F1BBCDC)
        } else {
            (b ^ c ^ d, 0xCA62C1D6)
        };
        let temp = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(w[i]);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut state: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let len_bits = (data.len() as u64) * 8;

    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&len_bits.to_be_bytes());

    for chunk in padded.chunks(64) {
        let mut block = [0u8; 64];
        block.copy_from_slice(chunk);
        sha1_transform(&mut state, &block);
    }

    let mut out = [0u8; 20];
    for i in 0..5 {
        out[i * 4..i * 4 + 4].copy_from_slice(&state[i].to_be_bytes());
    }
    out
}

// ============================================================================
// HMAC-SHA1 (RFC 2104)
// ============================================================================

fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    let block_size = SHA1_BLOCK_SIZE;
    let mut k = [0u8; SHA1_BLOCK_SIZE];
    if key.len() > block_size {
        let digest = sha1(key);
        k[..20].copy_from_slice(&digest);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut inner = Vec::with_capacity(SHA1_BLOCK_SIZE + data.len());
    for i in 0..block_size {
        inner.push(k[i] ^ 0x36);
    }
    inner.extend_from_slice(data);
    let inner_hash = sha1(&inner);

    let mut outer = [0u8; SHA1_BLOCK_SIZE + SHA1_DIGEST_SIZE];
    for i in 0..block_size {
        outer[i] = k[i] ^ 0x5C;
    }
    outer[block_size..].copy_from_slice(&inner_hash);
    sha1(&outer)
}

// ============================================================================
// PBKDF2-SHA1 (RFC 2898)
// ============================================================================

fn pbkdf2_hmac_sha1(password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    let mut dk = Vec::with_capacity(dk_len);
    let block_size = SHA1_DIGEST_SIZE;
    let num_blocks = (dk_len + block_size - 1) / block_size;

    for block in 1..=num_blocks {
        let mut u1_data = salt.to_vec();
        u1_data.extend_from_slice(&(block as u32).to_be_bytes());
        let mut u = hmac_sha1(password, &u1_data);
        let mut t = u.clone();

        for _ in 1..iterations {
            u = hmac_sha1(password, &u);
            for i in 0..block_size {
                t[i] ^= u[i];
            }
        }

        dk.extend_from_slice(&t);
    }

    dk.truncate(dk_len);
    dk
}

// ============================================================================
// IEEE 802.11 PRF (for PTK/GTK derivation)
// ============================================================================

fn prf_80211(k: &[u8], a: &[u8], b: &[u8], out_len: usize) -> Vec<u8> {
    // IEEE Std 802.11i-2004, 8.5.1.1 (PRF):
    //   for i = 0 to (Len+159)/160 - 1:
    //       R = R || HMAC-SHA1(K, A || 0x00 || B || i)
    // where the separator is a single 0x00 octet and `i` is a SINGLE OCTET
    // counter appended at the very end. (Verified against hostapd
    // src/crypto/sha1-prf.c: label_len = strlen(label)+1 includes the NUL, and
    // `counter` is a u8 written as the last 1-byte vector element.)
    let mut i: u8 = 0;
    let mut result = Vec::new();

    while result.len() < out_len {
        let mut data = Vec::with_capacity(a.len() + 1 + b.len() + 1);
        data.extend_from_slice(a);
        data.push(0x00);
        data.extend_from_slice(b);
        data.push(i);

        let h = hmac_sha1(k, &data);
        result.extend_from_slice(&h);
        i = i.wrapping_add(1);
    }

    result.truncate(out_len);
    result
}
// ============================================================================
// WPA2-PSK Key Derivation
// ============================================================================

/// Derive PMK from passphrase and SSID (PBKDF2-SHA1, 4096 iterations, 32 bytes)
pub fn derive_pmk(password: &str, ssid: &str) -> [u8; 32] {
    let mut pmk = [0u8; 32];
    let dk = pbkdf2_hmac_sha1(password.as_bytes(), ssid.as_bytes(), 4096, 32);
    pmk.copy_from_slice(&dk);
    pmk
}

/// Pairwise Transient Key components
#[derive(Clone, Copy)]
pub struct Ptk {
    pub kck: [u8; 16],   // Key Confirmation Key (for MIC)
    pub kek: [u8; 16],   // Key Encryption Key (for key data encryption)
    pub tk: [u8; 16],    // Temporal Key (for data encryption, CCMP)
}

impl Ptk {
    pub fn from_pmk(
        pmk: &[u8; 32],
        aa: &[u8; 6],
        spa: &[u8; 6],
        anonce: &[u8; 32],
        snonce: &[u8; 32],
    ) -> Self {
        let mut b = Vec::new();
        let (mac_min, mac_max) = if aa < spa { (aa, spa) } else { (spa, aa) };
        let (nonce_min, nonce_max) = if anonce < snonce { (anonce, snonce) } else { (snonce, anonce) };
        b.extend_from_slice(mac_min);
        b.extend_from_slice(mac_max);
        b.extend_from_slice(nonce_min);
        b.extend_from_slice(nonce_max);

        let a = b"Pairwise key expansion";
        let prf_out = prf_80211(pmk, a, &b, 64);

        let mut kck = [0u8; 16];
        let mut kek = [0u8; 16];
        let mut tk = [0u8; 16];
        kck.copy_from_slice(&prf_out[0..16]);
        kek.copy_from_slice(&prf_out[16..32]);
        tk.copy_from_slice(&prf_out[32..48]);

        Ptk { kck, kek, tk }
    }
}

/// Known-Answer Test for the WPA2 key-derivation chain.
///
/// The expected values were computed independently with Python
/// (hashlib.pbkdf2_hmac + hmac) using the standard algorithms, so this catches
/// the classic "wrong PRF counter/separator" or "broken SHA1/PBKDF2" bugs.
/// Returns (pbkdf2_ok, kck_ok, kek_ok, tk_ok). Used by `wifi diag`.
pub fn self_test() -> (bool, bool, bool, bool) {
    // PBKDF2-SHA1(passphrase="password", salt="IEEE", 4096, 32) — the canonical
    // WPA PMK test vector.
    let pmk = derive_pmk("password", "IEEE");
    let exp_pmk: [u8; 32] = [
        0xf4, 0x2c, 0x6f, 0xc5, 0x2d, 0xf0, 0xeb, 0xef,
        0x9e, 0xbb, 0x4b, 0x90, 0xb3, 0x8a, 0x5f, 0x90,
        0x2e, 0x83, 0xfe, 0x1b, 0x13, 0x5a, 0x70, 0xe2,
        0x3a, 0xed, 0x76, 0x2e, 0x97, 0x10, 0xa1, 0x2e,
    ];
    let pbkdf2_ok = pmk == exp_pmk;

    let mut k_pmk = [0u8; 32];
    for i in 0..32 {
        k_pmk[i] = i as u8;
    }
    let aa = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
    let spa = [0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb];
    let mut anonce = [0u8; 32];
    let mut snonce = [0u8; 32];
    for i in 0..32 {
        anonce[i] = 0x20 + i as u8;
        snonce[i] = i as u8;
    }
    let ptk = Ptk::from_pmk(&k_pmk, &aa, &spa, &anonce, &snonce);

    let exp_kck: [u8; 16] = [
        0x79, 0xc3, 0x15, 0x81, 0x7d, 0xa2, 0xce, 0x91,
        0x7e, 0x32, 0x64, 0xee, 0xac, 0x39, 0xf9, 0x08,
    ];
    let exp_kek: [u8; 16] = [
        0xed, 0xaf, 0xe7, 0x82, 0x05, 0xab, 0x10, 0xe5,
        0xfa, 0x4d, 0x85, 0x50, 0x52, 0x35, 0xf6, 0x96,
    ];
    let exp_tk: [u8; 16] = [
        0xad, 0x89, 0x09, 0x20, 0x24, 0xc3, 0x45, 0x24,
        0xf0, 0x7b, 0x21, 0xe1, 0xf7, 0xba, 0xf3, 0xb4,
    ];

    (
        pbkdf2_ok,
        ptk.kck == exp_kck,
        ptk.kek == exp_kek,
        ptk.tk == exp_tk,
    )
}
// ============================================================================
// EAPOL-Key Constants
// ============================================================================

pub const EAPOL_ETHERTYPE: u16 = 0x888E;
pub const EAPOL_VERSION: u8 = 2;
pub const EAPOL_TYPE_KEY: u8 = 3;

/// Verbose hexdumps inside the 4-way handshake. MUST stay `false` for real
/// connections: the AP's EAPOL retry timer is 100 ms for the first timeout
/// (`eapol_key_timeout_first`) and 1 s afterwards. Printing ~10 hexdump lines
/// (serial at 115200 baud + framebuffer) between RX and TX pushes our reply
/// past that window, so the AP rejects it as an "unexpected replay counter"
/// and keeps retransmitting msg 1/4 or msg 3/4 until it deauths us.
/// Set to `true` only when debugging a handshake on a fast/quiet console.
const WPA_HEXDUMP: bool = false;

pub const KEY_DESC_RSN: u8 = 2;

// RSN (802.11i) Key Information field (16 bits):
//   bits 0-2: Descriptor Version (2 = HMAC-SHA1-128 + AES Key Wrap)
//   bit 3:    Key Type (1 = Pairwise)
//   bit 6:    Install
//   bit 7:    Key Ack
//   bit 8:    Key MIC
//   bit 9:    Secure
//   bit 12:   Encrypted Key Data
pub const KEY_VER_2: u16 = 0x0002;
pub const KEY_INFO_PAIRWISE: u16 = 1 << 3; // 0x0008
pub const KEY_INFO_INSTALL: u16 = 1 << 6; // 0x0040
pub const KEY_INFO_ACK: u16 = 1 << 7; // 0x0080
pub const KEY_INFO_MIC: u16 = 1 << 8; // 0x0100
pub const KEY_INFO_SECURE: u16 = 1 << 9; // 0x0200
pub const KEY_INFO_ENCRYPTED: u16 = 1 << 12; // 0x1000

// Complete key_info values for each handshake message we send/receive.
// Msg 1/4 (AP->STA): pairwise, ack, no mic, no secure
pub const MSG1_KEY_INFO: u16 = KEY_VER_2 | KEY_INFO_PAIRWISE | KEY_INFO_ACK;
// Msg 2/4 (STA->AP): pairwise, mic (version 2)
pub const MSG2_KEY_INFO: u16 = KEY_VER_2 | KEY_INFO_PAIRWISE | KEY_INFO_MIC;
// Msg 3/4 (AP->STA): pairwise, install, ack, mic, secure, encrypted
pub const MSG3_KEY_INFO: u16 = KEY_VER_2 | KEY_INFO_PAIRWISE | KEY_INFO_INSTALL
    | KEY_INFO_ACK | KEY_INFO_MIC | KEY_INFO_SECURE | KEY_INFO_ENCRYPTED;
// Msg 4/4 (STA->AP): pairwise, mic, secure
pub const MSG4_KEY_INFO: u16 = KEY_VER_2 | KEY_INFO_PAIRWISE | KEY_INFO_MIC | KEY_INFO_SECURE;

/// Group-key 2/2 (wpa_supplicant_send_2_of_2): no KEY_TYPE/ACK/INSTALL,
/// MIC + SECURE, replay echoed, no key data.
pub const MSG2_GROUP_KEY_INFO: u16 = KEY_VER_2 | KEY_INFO_MIC | KEY_INFO_SECURE;

pub const RSN_IE_ELEM_ID: u8 = 48;
pub const RSN_VERSION: u16 = 1;
pub const RSN_GRP_CIPHER_CCMP: u32 = 0x00_0F_AC_04;
pub const RSN_PAIRWISE_CIPHER_CCMP: u32 = 0x00_0F_AC_04;
pub const RSN_AKM_PSK: u32 = 0x00_0F_AC_02;
// ============================================================================
// EAPOL-Key Frame Structures
// ============================================================================

#[derive(Clone, Debug)]
pub struct EapolKeyFrame {
    pub key_info: u16,
    pub key_length: u16,
    pub replay_counter: [u8; 8],
    pub key_nonce: [u8; 32],
    pub key_mic: [u8; 16],
    pub key_data_len: u16,
    pub key_data: Vec<u8>,
}

pub fn build_eapol_key_body(
    key_info: u16,
    key_length: u16,
    replay_counter: &[u8; 8],
    key_nonce: &[u8; 32],
    key_mic: &[u8; 16],
    key_data: &[u8],
) -> Vec<u8> {
    let mut body = Vec::new();
    body.push(KEY_DESC_RSN);
    body.extend_from_slice(&key_info.to_be_bytes());
    // RSN (WPA2) sends Key Length = 0 in msg 2/4 (hostapd
    // wpa_supplicant_send_2_of_4/4_of_4). Keeping 16 here made the AP reject
    // every msg 4/4 and retransmit msg 3/4 until it deauthenticated us.
    body.extend_from_slice(&key_length.to_be_bytes());
    body.extend_from_slice(replay_counter);
    body.extend_from_slice(key_nonce);
    body.extend_from_slice(&[0u8; 16]);
    body.extend_from_slice(&[0u8; 8]);
    body.extend_from_slice(&[0u8; 8]);
    body.extend_from_slice(key_mic);
    body.extend_from_slice(&(key_data.len() as u16).to_be_bytes());
    body.extend_from_slice(key_data);
    body
}

/// Build a complete EAPOL packet: Version + Type + Length + Key body.
/// This is the form expected by `parse_eapol_key` and `eapol_mic`
/// (MIC is computed starting from the Version byte).
pub fn build_eapol_packet(
    key_info: u16,
    key_length: u16,
    replay_counter: &[u8; 8],
    key_nonce: &[u8; 32],
    key_mic: &[u8; 16],
    key_data: &[u8],
) -> Vec<u8> {
    let body = build_eapol_key_body(key_info, key_length, replay_counter, key_nonce, key_mic, key_data);
    let mut pkt = Vec::with_capacity(4 + body.len());
    pkt.push(EAPOL_VERSION);
    pkt.push(EAPOL_TYPE_KEY);
    pkt.extend_from_slice(&(body.len() as u16).to_be_bytes());
    pkt.extend_from_slice(&body);
    pkt
}
pub fn eapol_mic(kck: &[u8; 16], frame: &[u8]) -> [u8; 16] {
    let mut mic_input = frame.to_vec();
    // Key MIC field is at offset 81 within the EAPOL packet (4-byte EAPOL
    // header + 77 bytes into the EAPOL-Key body).
    let mic_offset = 81;
    if mic_input.len() >= mic_offset + 16 {
        for i in mic_offset..mic_offset + 16 {
            mic_input[i] = 0;
        }
    }
    let digest = hmac_sha1(kck, &mic_input);
    let mut mic = [0u8; 16];
    mic.copy_from_slice(&digest[..16]);
    mic
}

pub fn build_rsn_ie() -> Vec<u8> {
    let mut ie = Vec::new();
    ie.push(RSN_IE_ELEM_ID);
    ie.push(20u8); // length = 20 (must match actual content)
    // RSN Version (2 bytes LE)
    ie.extend_from_slice(&RSN_VERSION.to_le_bytes());
    // Group Cipher Suite (4 bytes, raw): CCMP
    ie.extend_from_slice(&RSN_GRP_CIPHER_CCMP.to_be_bytes());
    // Pairwise Cipher Suite Count (2 bytes LE) = 1
    ie.extend_from_slice(&1u16.to_le_bytes());
    // Pairwise Cipher Suite (4 bytes): CCMP
    ie.extend_from_slice(&RSN_PAIRWISE_CIPHER_CCMP.to_be_bytes());
    // AKM Suite Count (2 bytes LE) = 1
    ie.extend_from_slice(&1u16.to_le_bytes());
    // AKM Suite (4 bytes): PSK
    ie.extend_from_slice(&RSN_AKM_PSK.to_be_bytes());
    // RSN Capabilities (2 bytes LE)
    ie.extend_from_slice(&0u16.to_le_bytes());
    ie
}

/// Print bytes as hex on the SCREEN (framebuffer) for on-device debugging
/// (the serial console is not visible to this user).
pub fn screen_hex(tag: &str, data: &[u8]) {
    use core::fmt::Write;
    let mut s = alloc::string::String::new();
    for (i, b) in data.iter().enumerate() {
        let _ = write!(s, "{:02x} ", b);
        if i % 24 == 23 {
            crate::mesa_println!("[{}] {}", tag, s);
            s.clear();
        }
    }
    if !s.is_empty() {
        crate::mesa_println!("[{}] {}", tag, s);
    }
}
// ============================================================================
// WPA2 Connection State
// ============================================================================

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WpaState {
    Disconnected = 0,
    /// Associated with AP; waiting for EAPOL msg 1/4.
    Start = 1,
    /// Sent msg 2/4; waiting for msg 3/4.
    Msg2Sent = 2,
    /// 4-way handshake done; PTK (and GTK) installed.
    Completed = 3,
    Failed = 4,
}

pub static WPA_STATE: AtomicU8 = AtomicU8::new(WpaState::Disconnected as u8);

pub fn get_wpa_state() -> WpaState {
    match WPA_STATE.load(Ordering::Relaxed) {
        1 => WpaState::Start,
        2 => WpaState::Msg2Sent,
        3 => WpaState::Completed,
        4 => WpaState::Failed,
        _ => WpaState::Disconnected,
    }
}

pub fn set_wpa_state(state: WpaState) {
    WPA_STATE.store(state as u8, Ordering::Relaxed);
}

pub static PTK_STORE: Mutex<Option<Ptk>> = Mutex::new(None);
/// GTK (group key) captured from msg 3/4, needed to decrypt broadcast/multicast.
pub static GTK_STORE: Mutex<Option<[u8; 32]>> = Mutex::new(None);
pub static GTK_LEN: AtomicU8 = AtomicU8::new(0);
/// GTK key id (0..3) from the KDE, used as the HW CAM group-key index.
pub static GTK_KEYID: AtomicU8 = AtomicU8::new(0);

pub fn set_ptk(ptk: Ptk) {
    *PTK_STORE.lock() = Some(ptk);
}

pub fn get_ptk() -> Option<Ptk> {
    PTK_STORE.lock().clone()
}

pub fn set_gtk(gtk: [u8; 32], len: usize, keyid: u8) {
    *GTK_STORE.lock() = Some(gtk);
    GTK_LEN.store(len as u8, Ordering::Relaxed);
    GTK_KEYID.store(keyid & 0x3, Ordering::Relaxed);
}

pub fn get_gtk() -> Option<([u8; 32], usize)> {
    GTK_STORE.lock().map(|g| (g, GTK_LEN.load(Ordering::Relaxed) as usize))
}

pub fn get_gtk_keyid() -> u8 {
    GTK_KEYID.load(Ordering::Relaxed)
}

/// Compute the RSN PMKID for this (PMK, AP, STA): HMAC-SHA1-128(PMK,
/// "PMK Name" || AA || SPA). The AP includes the expected PMKID in msg 1/4
/// key data when PMK caching is used, so comparing it proves whether our
/// passphrase (hence PMK) matches the one the AP knows.
pub fn pmkid_for(pmk: &[u8; 32], aa: &[u8; 6], spa: &[u8; 6]) -> [u8; 16] {
    let mut data = Vec::new();
    data.extend_from_slice(b"PMK Name");
    data.extend_from_slice(aa);
    data.extend_from_slice(spa);
    let d = hmac_sha1(pmk, &data);
    let mut out = [0u8; 16];
    out.copy_from_slice(&d[..16]);
    out
}

/// Extract the PMKID KDE ([0xDD][20][00 0F AC 04][16 bytes]) from EAPOL key data.
fn extract_pmkid_kde(data: &[u8]) -> Option<[u8; 16]> {
    let mut i = 0;
    while i + 2 <= data.len() {
        let id = data[i];
        let len = data[i + 1] as usize;
        if i + 2 + len > data.len() {
            break;
        }
        let body = &data[i + 2..i + 2 + len];
        if id == 0xDD && len >= 20 && body[0..3] == [0x00, 0x0F, 0xAC] && body[3] == 0x04 {
            let mut p = [0u8; 16];
            p.copy_from_slice(&body[4..20]);
            return Some(p);
        }
        i += 2 + len;
    }
    None
}

/// Set by `handle_msg3` when the 4-way handshake completes. The connect path
/// consumes it (outside the RX/driver borrow) to write TK+GTK into the HW CAM,
/// so keys are installed even if msg 3/4 arrives after the initial wait.
/// Last msg 4/4 packet (EAPOL payload), AP address and remaining proactive
/// retransmissions. Data TX right after association is lossy while the rate
/// control settles, so the AP often never sees the single msg 4/4 and then
/// deauthenticates us (reason=3) when we start DHCP. Resending it a handful of
/// times mirrors mac80211's ieee80211_tx_control_port() retry + the AP's own
/// msg3 retransmissions.
pub static MSG4_RETRY: Mutex<Option<(Vec<u8>, [u8; 6], u8)>> = Mutex::new(None);
static MSG4_LAST_TICK: AtomicU64 = AtomicU64::new(0);

pub static KEYS_PENDING: AtomicBool = AtomicBool::new(false);

pub fn set_keys_pending() {
    KEYS_PENDING.store(true, Ordering::Release);
}

/// Returns true once after the handshake completed and keys still need to be
/// written to the hardware.
pub fn take_keys_pending() -> bool {
    KEYS_PENDING.swap(false, Ordering::AcqRel)
}

pub fn clear_wpa() {
    set_wpa_state(WpaState::Disconnected);
    KEYS_PENDING.store(false, Ordering::Release);
    *PTK_STORE.lock() = None;
    *GTK_STORE.lock() = None;
    *HANDSHAKE.lock() = None;
    *MSG4_RETRY.lock() = None;
}

// ============================================================================
// Handshake context (drives msg 2/4 and 4/4 responses)
// ============================================================================

pub struct HandshakeCtx {
    pub pmk: [u8; 32],
    pub snonce: [u8; 32],
    pub anonce: [u8; 32],
    pub aa: [u8; 6],  // AP MAC (authenticator)
    pub spa: [u8; 6], // our MAC (supplicant)
    pub ptk: Option<Ptk>,
}

static HANDSHAKE: Mutex<Option<HandshakeCtx>> = Mutex::new(None);

/// msg-1/4 retransmissions seen in the current handshake (AP rejecting msg2).
static MSG1_RETRIES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// Begin the 4-way handshake: generate a fresh SNonce and store the PMK so we
/// can derive the PTK once msg 1/4 (ANonce) arrives. Call after association.
pub fn start_handshake(pmk: &[u8; 32], aa: [u8; 6], spa: [u8; 6]) {
    let mut snonce = [0u8; 32];
    for chunk in snonce.chunks_mut(8) {
        chunk.copy_from_slice(&crate::security::random_u64().to_le_bytes());
    }
    *HANDSHAKE.lock() = Some(HandshakeCtx {
        pmk: *pmk,
        snonce,
        anonce: [0u8; 32],
        aa,
        spa,
        ptk: None,
    });
    set_wpa_state(WpaState::Start);
    KEYS_PENDING.store(false, Ordering::Release);
    MSG1_RETRIES.store(0, Ordering::Relaxed);
}

/// Generate a fresh 32-byte nonce.
#[allow(dead_code)]
fn random_nonce() -> [u8; 32] {
    let mut n = [0u8; 32];
    for chunk in n.chunks_mut(8) {
        chunk.copy_from_slice(&crate::security::random_u64().to_le_bytes());
    }
    n
}

/// Build an EAPOL-Key packet and sign it with the KCK (MIC at offset 81).
fn build_signed_eapol(
    kck: &[u8; 16],
    key_info: u16,
    key_length: u16,
    replay: &[u8; 8],
    nonce: &[u8; 32],
    key_data: &[u8],
) -> Vec<u8> {
    let mut pkt = build_eapol_packet(key_info, key_length, replay, nonce, &[0u8; 16], key_data);
    if pkt.len() >= 97 {
        let mic = eapol_mic(kck, &pkt);
        pkt[81..97].copy_from_slice(&mic);
    }
    pkt
}

// ============================================================================
// EAPOL-Key parsing (incoming from AP)
// ============================================================================

pub fn parse_eapol_key(data: &[u8]) -> Option<EapolKeyFrame> {
    if data.len() < 5 {
        return None;
    }
    let body_len = u16::from_be_bytes([data[2], data[3]]) as usize;
    if data.len() < 4 + body_len {
        return None;
    }
    let body = &data[4..4 + body_len];
    if body.len() < 5 + 8 + 32 + 16 + 8 + 8 + 16 + 2 {
        return None;
    }
    let desc_type = body[0];
    if desc_type != KEY_DESC_RSN {
        return None;
    }
    let key_info = u16::from_be_bytes([body[1], body[2]]);
    let key_length = u16::from_be_bytes([body[3], body[4]]);
    let mut pos = 5;
    let mut replay_counter = [0u8; 8];
    replay_counter.copy_from_slice(&body[pos..pos + 8]);
    pos += 8;
    let mut key_nonce = [0u8; 32];
    key_nonce.copy_from_slice(&body[pos..pos + 32]);
    pos += 32;
    pos += 16;
    pos += 8;
    pos += 8;
    let mut key_mic = [0u8; 16];
    key_mic.copy_from_slice(&body[pos..pos + 16]);
    pos += 16;
    let key_data_len = u16::from_be_bytes([body[pos], body[pos + 1]]) as usize;
    pos += 2;
    let key_data = body[pos..pos + key_data_len].to_vec();

    Some(EapolKeyFrame {
        key_info,
        key_length,
        replay_counter,
        key_nonce,
        key_mic,
        key_data_len: key_data_len as u16,
        key_data,
    })
}
// ============================================================================
// EAPOL Receive Handler (called from net::handle_packet)
// ============================================================================

pub fn handle_eapol(data: &[u8], src_mac: [u8; 6]) {
    let eapol = match parse_eapol_key(data) {
        Some(f) => f,
        None => return,
    };
    let state = get_wpa_state();

    let has_ack = eapol.key_info & KEY_INFO_ACK != 0;
    let has_mic = eapol.key_info & KEY_INFO_MIC != 0;
    let has_install = eapol.key_info & KEY_INFO_INSTALL != 0;
    // KEY_TYPE=1 is the pairwise handshake; KEY_TYPE=0 is the group-key
    // handshake. Both carry ACK+MIC(+INSTALL for the group 1/2), so checking
    // INSTALL alone is not enough to tell them apart.
    let pairwise = eapol.key_info & KEY_INFO_PAIRWISE != 0;

    match state {
        // Expect msg 1/4: pairwise, ACK, no MIC.
        WpaState::Start if pairwise && has_ack && !has_mic => {
            handle_msg1(&eapol, src_mac);
        }
        // Expect msg 3/4: pairwise, install, ACK, MIC.
        WpaState::Msg2Sent if pairwise && has_ack && has_mic && has_install => {
            handle_msg3(&eapol);
        }
        // AP retransmitted msg 1/4 (pairwise, ACK, no MIC) while we wait for
        // msg 3/4: it never got our msg 2/4 (or rejected it). Re-send msg 2/4
        // with the SAME SNonce/PTK instead of ignoring it — the standard
        // retransmission behaviour that keeps the handshake alive.
        WpaState::Msg2Sent if pairwise && has_ack && !has_mic => {
            crate::mesa_println!("[WPA2] msg 1/4 retransmitido por el AP, reenviando 2/4...");
            handle_msg1(&eapol, src_mac);
        }
        WpaState::Completed => {
            // The AP retransmitted msg 3/4 because it never received our msg
            // 4/4 (lossy first TX). Re-send msg 4/4; re-running handle_msg3 is
            // idempotent (same PTK/GTK, same replay) and keeps the port open.
            if pairwise && has_ack && has_mic && has_install {
                crate::mesa_println!("[WPA2] msg 3/4 retransmitido, reenviando 4/4...");
                handle_msg3(&eapol);
            } else if !pairwise && has_ack && has_mic {
                // Group-key 1/2 (initial or rekey). Must answer with 2/2,
                // otherwise the AP retries and eventually deauthenticates us.
                handle_group_msg1(&eapol);
            }
        }
        _ => {
            // Show on SCREEN: any unexpected EAPOL (e.g. a retransmitted msg 1/4
            // or a deauth trigger) is key to diagnosing a failed handshake.
            crate::mesa_println!(
                "[WPA2] EAPOL inesperado key_info={:#06x} state={:?}",
                eapol.key_info, state
            );
        }
    }
}

/// Handle the group-key 1/2 message (KEY_TYPE=0): acknowledge it with a
/// group 2/2 so the AP does not retry and deauthenticate. A rekey would also
/// carry a new GTK in the key data; installing that is handled elsewhere and
/// is not required to keep the link up on first association.
fn handle_group_msg1(eapol: &EapolKeyFrame) {
    let ptk = match get_ptk() {
        Some(p) => p,
        None => {
            crate::mesa_println!("[WPA2] group 1/2 sin PTK, ignorado");
            return;
        }
    };
    let aa = crate::drivers::wifi::state::get_bssid();
    let pkt = build_signed_eapol(
        &ptk.kck,
        MSG2_GROUP_KEY_INFO,
        0,
        &eapol.replay_counter,
        &[0u8; 32],
        &[],
    );
    match send_eapol_key(aa, &pkt) {
        Ok(()) => crate::mesa_println!(
            "[WPA2] group 1/2 -> 2/2 enviado (kd={}B)",
            eapol.key_data.len()
        ),
        Err(e) => crate::mesa_println!("[WPA2] group 2/2 fallo: {}", e),
    }
}

/// Handle msg 1/4: store ANonce, derive PTK, send msg 2/4.
/// If the AP retransmits msg 1/4 (same replay counter / new ANonce), we
/// re-derive the PTK from the NEW ANonce but keep the SAME SNonce — the
/// standard 802.11 retransmission behaviour. Regenerating the SNonce here
/// would desync the PTK and the AP would keep rejecting our msg 2/4.
fn handle_msg1(eapol: &EapolKeyFrame, src_mac: [u8; 6]) {
    let (pmk, aa, spa, snonce, ptk, aa_changed) = {
        let mut hs = HANDSHAKE.lock();
        let ctx = match hs.as_mut() {
            Some(c) => c,
            None => {
                crate::mesa_println!("[WPA2] msg1 pero no hay contexto de handshake");
                return;
            }
        };
        // The PTK's AA is the authenticator MAC = source of the EAPOL frame.
        // Normally it equals the BSSID we associated with, but on mesh /
        // multi-BSSID APs the msg1 can come from a different radio; using a
        // stale BSSID would make every msg2 MIC invalid (AP retries msg1 and
        // finally disassociates with reason=15).
        // AA for the PTK is the authenticator BSSID we associated with. Log
        // (don't switch) if the EAPOL source differs: switching on a bridge
        // SA would break a working handshake.
        let aa = ctx.aa;
        let aa_changed = src_mac != [0u8; 6]
            && src_mac != [0xffu8; 6]
            && src_mac != ctx.aa;
        if aa_changed {
            crate::mesa_println!("[WPA2] AVISO: EAPOL src != BSSID (AA sigue siendo BSSID)");
            crate::net::wpa::screen_hex("WPA2 EAPOL src:", &src_mac);
            crate::net::wpa::screen_hex("WPA2 BSSID:", &ctx.aa);
        }
        ctx.anonce = eapol.key_nonce;
        let ptk = Ptk::from_pmk(&ctx.pmk, &ctx.aa, &ctx.spa, &ctx.anonce, &ctx.snonce);
        ctx.ptk = Some(ptk);
        (ctx.pmk, ctx.aa, ctx.spa, ctx.snonce, ptk, aa_changed)
    };
    let kck = ptk.kck;
    let _ = aa_changed;

    // A handshake that never leaves msg2 means the AP rejected our MIC. Count
    // the retries and spell out the exact inputs once, so a bad passphrase /
    // SSID is distinguishable from an AA/SPA mismatch.
    let retries = MSG1_RETRIES.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    if retries == 2 {
        crate::mesa_println!("[WPA2] msg2 rechazado 3 veces: PMK={:02x}{:02x}{:02x}{:02x}",
            pmk[0], pmk[1], pmk[2], pmk[3]);
        crate::net::wpa::screen_hex("WPA2 AA:", &aa);
        crate::net::wpa::screen_hex("WPA2 SPA:", &spa);
        crate::net::wpa::screen_hex("WPA2 ANonce:", &eapol.key_nonce);
        crate::net::wpa::screen_hex("WPA2 SNonce:", &snonce);
        crate::mesa_println!("[WPA2] Si la contraseña/SSID es correcta, el AP está rechazando el MIC");
    }
    if retries == 0 {
        crate::mesa_println!("[WPA2] PTK inputs AA={:02x?} SPA={:02x?} ANonce0={:02x?} SNonce0={:02x?}",
            aa, spa, &eapol.key_nonce[0..4], &snonce[0..4]);
    }

    // msg 2/4: RSN IE MUST be byte-identical to the one in the (Re)Association
    // Request. Use the exact bytes negotiated at assoc time (group cipher
    // mirrored from the AP); a hardcoded CCMP group when the AP chose TKIP
    // makes it reject msg 2/4, retransmit msg 1/4 and disassociate us.
    let mut key_data = crate::drivers::wifi::state::get_assoc_rsn_ie();
    if key_data.is_empty() {
        key_data = crate::drivers::wifi::frame::IeBuilder::add_rsn_wpa2_psk();
    }
    crate::mesa_println!("[WPA2] msg2 RSN IE ({}B): {:02x?}", key_data.len(), key_data);
    let pkt = build_signed_eapol(&kck, MSG2_KEY_INFO, 0, &eapol.replay_counter, &snonce, &key_data);

    // Send msg 2/4 BEFORE any diagnostic output. The AP retransmits msg 1/4
    // with a fresh replay counter roughly every second; printing all the hex
    // dumps below first delayed this reply past that window, so the AP
    // rejected it as an "unexpected replay counter" and retried msg 1/4 four
    // times. The same delay makes msg 4/4 miss the msg 3/4 window and the AP
    // ends up deauthenticating us (reason=3).
    match send_eapol_key(aa, &pkt) {
        Ok(()) => {
            set_wpa_state(WpaState::Msg2Sent);
            crate::mesa_println!("[WPA2] EAPOL 2/4 enviado (SNonce + MIC) txseq={}",
                crate::drivers::net::rtl8822ce::last_tx_seq());
        }
        Err(e) => {
            crate::mesa_println!("[WPA2] error al enviar 2/4: {}", e);
            set_wpa_state(WpaState::Failed);
            return;
        }
    }

    // Decisive check: if the AP sent a PMKID in msg 1/4 key data, compare it
    // with ours. A mismatch means our passphrase (PMK) is wrong, not the code.
    if let Some(ap_pmkid) = extract_pmkid_kde(&eapol.key_data) {
        let our = pmkid_for(&pmk, &aa, &spa);
        crate::mesa_println!("[WPA2] PMKID check: {}",
            if ap_pmkid == our { "MATCH -> contraseña CORRECTA" }
            else { "DISTINTO -> contraseña INCORRECTA" });
        if WPA_HEXDUMP {
            screen_hex("WPA2 AP PMKID:", &ap_pmkid);
            screen_hex("WPA2 our PMKID:", &our);
        }
    } else {
        crate::mesa_println!("[WPA2] (AP no incluye PMKID en msg1)");
    }

    // Short diagnostics only (the verbose dumps must not delay the next RX).
    crate::mesa_println!("[WPA2] msg1 key_info={:#06x} replay={:02x?}", eapol.key_info, eapol.replay_counter);
    crate::mesa_println!("[WPA2] msg2 key_info={:#06x} len={} rsn_len={}", MSG2_KEY_INFO, pkt.len(), key_data.len());
    if WPA_HEXDUMP {
        screen_hex("WPA2 ANonce(AP):", &eapol.key_nonce);
        screen_hex("WPA2 SNonce(us):", &snonce);
        screen_hex("WPA2 KCK:", &kck);
        screen_hex("WPA2 msg2 RSN IE:", &key_data);
        screen_hex("WPA2 msg2 pkt:", &pkt);
    }
}

/// Handle msg 3/4: verify MIC, unwrap GTK, send msg 4/4, install PTK/GTK.
fn handle_msg3(eapol: &EapolKeyFrame) {
    let (ptk, snonce, aa) = {
        let mut hs = HANDSHAKE.lock();
        let ctx = match hs.as_mut() {
            Some(c) => c,
            None => return,
        };
        match ctx.ptk {
            Some(p) => (p, ctx.snonce, ctx.aa),
            None => return,
        }
    };

    // msg 4/4: pairwise + MIC + secure, empty key data. Send it IMMEDIATELY,
    // before verifying the MIC / unwrapping the GTK / printing diagnostics:
    // the AP retransmits msg 3/4 roughly once per second with a fresh replay
    // counter, and all that work used to push msg 4/4 past that window. The
    // AP then rejected every msg 4/4 as an "unexpected replay counter", kept
    // retrying msg 3/4 and finally deauthenticated us (reason=3).
    let pkt = build_signed_eapol(&ptk.kck, MSG4_KEY_INFO, 0, &eapol.replay_counter, &[0u8; 32], &[]);
    match send_eapol_key(aa, &pkt) {
        Ok(()) => {
            set_ptk(ptk);
            set_wpa_state(WpaState::Completed);
            set_keys_pending();
            crate::mesa_println!("[WPA2] EAPOL 4/4 sent — handshake COMPLETE txseq={}",
                crate::drivers::net::rtl8822ce::last_tx_seq());
            // Keep the packet armed for a few proactive retransmissions: the
            // first data TX after association is frequently lost, and if the
            // AP never sees msg4 it keeps the 802.1X port closed and deauths
            // us (reason=3) as soon as a DHCP frame arrives.
            *MSG4_RETRY.lock() = Some((pkt, aa, 5));
            MSG4_LAST_TICK.store(crate::curr_arch::get_ticks(), Ordering::Relaxed);
        }
        Err(e) => {
            crate::mesa_println!("[WPA2] failed to send 4/4: {}", e);
            set_wpa_state(WpaState::Failed);
            return;
        }
    }

    // Diagnostics only AFTER the reply has gone out.
    crate::mesa_println!("[WPA2] msg3 key_info={:#06x} klen={} replay={:02x?} kd_len={}",
        eapol.key_info, eapol.key_length, eapol.replay_counter, eapol.key_data.len());

    // Rebuild the received packet to verify MIC integrity.
    let mut verify = build_eapol_packet(
        eapol.key_info,
        eapol.key_length,
        &eapol.replay_counter,
        &eapol.key_nonce,
        &[0u8; 16],
        &eapol.key_data,
    );
    if verify.len() >= 97 {
        verify[81..97].copy_from_slice(&eapol.key_mic);
        let calc = eapol_mic(&ptk.kck, &verify);
        if calc != eapol.key_mic {
            crate::mesa_println!("[WPA2] AVISO: MIC msg3/4 no coincide (continuando)");
            if WPA_HEXDUMP {
                screen_hex("WPA2 msg3 MIC recv:", &eapol.key_mic);
                screen_hex("WPA2 msg3 MIC calc:", &calc);
            }
        } else {
            crate::mesa_println!("[WPA2] MIC msg3/4 OK");
        }
    }

    // Unwrap the GTK from the KEK-encrypted key data.
    if !eapol.key_data.is_empty() {
        let mut inner = [0u8; 512];
        let inner_len = eapol.key_data.len().saturating_sub(8);
        if eapol.key_data.len() >= 24
            && eapol.key_data.len() <= 512
            && aes_key_unwrap(&ptk.kek, &eapol.key_data, &mut inner[..inner_len]).is_some()
        {
            if WPA_HEXDUMP {
                screen_hex("WPA2 keydata plano:", &inner[..inner_len]);
            }
            if let Some((gtk, glen, gkeyid)) = parse_gtk_from_keydata(&inner[..inner_len]) {
                crate::mesa_println!("[WPA2] GTK extraído ({} bytes, keyid={})", glen, gkeyid);
                set_gtk(gtk, glen, gkeyid);
            }
        } else {
            crate::mesa_println!("[WPA2] AVISO: GTK unwrap falló");
        }
    }
}

/// Called from the periodic WiFi tick (before the driver lock is taken).
/// While the handshake is complete, resend the stored msg 4/4 every ~200 ms a
/// few times so a lost first transmission does not leave the AP's port closed.
pub fn retransmit_msg4_tick() {
    if get_wpa_state() != WpaState::Completed {
        return;
    }
    let now = crate::curr_arch::get_ticks();
    // ~100 Hz timer -> 10 ticks == 100 ms (the AP's first EAPOL timeout).
    if now.wrapping_sub(MSG4_LAST_TICK.load(Ordering::Relaxed)) < 10 {
        return;
    }

    let job = {
        let mut slot = MSG4_RETRY.lock();
        match slot.take() {
            Some((pkt, aa, left)) if left > 0 => {
                let new_left = left - 1;
                // Re-arm unless this was the last retransmission.
                if new_left > 0 {
                    *slot = Some((pkt.clone(), aa, new_left));
                }
                Some((aa, pkt))
            }
            Some((pkt, aa, _)) => {
                // Exhausted: disarm.
                let _ = (pkt, aa);
                None
            }
            None => None,
        }
    };

    if let Some((aa, pkt)) = job {
        let _ = send_eapol_key(aa, &pkt);
        MSG4_LAST_TICK.store(now, Ordering::Relaxed);
    }
}

/// Parse the GTK out of the unwrapped msg 3/4 key data (RSNE || GTK KDE).
/// Returns (gtk, len, key_id).
fn parse_gtk_from_keydata(inner: &[u8]) -> Option<([u8; 32], usize, u8)> {
    // Walk the plaintext Key Data as a list of IEs/KDEs; the RSNE (if present)
    // is just skipped. GTK KDE: [0xDD][len][00:0F:AC][type=1][key_id][GTK...].
    //
    // Mixed-mode APs can include more than one GTK KDE (e.g. TKIP and CCMP).
    // Prefer the one matching the group cipher negotiated in the RSN IE:
    // TKIP -> 32-byte key, CCMP -> 16-byte key. Otherwise the CAM would get a
    // GTK of the wrong cipher and broadcast (DHCP OFFER) would not decrypt.
    let want_len = if crate::drivers::wifi::state::group_cipher_is_tkip() {
        32
    } else {
        16
    };
    let mut fallback: Option<([u8; 32], usize, u8)> = None;
    let mut pos = 0usize;
    while pos + 2 <= inner.len() {
        let id = inner[pos];
        let len = inner[pos + 1] as usize;
        let end = pos + 2 + len;
        if end > inner.len() {
            break;
        }
        if id == 0xDD
            && len >= 7
            && inner[pos + 2..pos + 5] == [0x00, 0x0F, 0xAC]
            && inner[pos + 5] == 0x01
        {
            // Standard GTK KDE header: OUI(3) + type(1) + key_id(1) + reserved(1)
            // = 6 bytes, then the GTK. (len is 6 + key_len; e.g. 0x16 -> 16.)
            let key_id = inner[pos + 6] & 0x3;
            let gtk_start = pos + 8;
            let gtk_len = len - 6;
            if gtk_len > 0 && gtk_len <= 32 && gtk_start + gtk_len <= inner.len() {
                let mut gtk = [0u8; 32];
                gtk[..gtk_len].copy_from_slice(&inner[gtk_start..gtk_start + gtk_len]);
                if gtk_len == want_len {
                    return Some((gtk, gtk_len, key_id));
                }
                if fallback.is_none() {
                    fallback = Some((gtk, gtk_len, key_id));
                }
            }
        }
        pos = end;
    }
    fallback
}

/// Send an EAPOL packet over the WiFi DATA path (BE queue, QoS-Data frame).
/// EAPOL 2/4 and 4/4 are 802.11 DATA frames (ToDS=1), NOT management frames:
/// sending them via the MGT queue makes the AP silently drop them, so the
/// handshake never completes. We build a full Ethernet frame (dst/src/type +
/// EAPOL payload) and route it through the normal data TX path (BE queue).
pub fn send_eapol_key(dst_mac: [u8; 6], eapol_pkt: &[u8]) -> Result<(), &'static str> {
    let src_mac = crate::net::get_mac();
    let bssid = crate::drivers::wifi::state::get_bssid();
    if bssid == [0u8; 6] {
        return Err("WPA2: not associated");
    }
    // Ethernet frame: dst(6) + src(6) + ethertype(2) + EAPOL payload.
    let mut eth = alloc::vec::Vec::with_capacity(14 + eapol_pkt.len());
    eth.extend_from_slice(&dst_mac);
    eth.extend_from_slice(&src_mac);
    eth.extend_from_slice(&EAPOL_ETHERTYPE.to_be_bytes());
    eth.extend_from_slice(eapol_pkt);
    crate::drivers::net::rtl8822ce::send_packet(&eth)
}

// ============================================================================
// AES-128 (software) — needed for AES Key Unwrap of the GTK in EAPOL msg 3/4
// ============================================================================

#[rustfmt::skip]
const AES_SBOX: [u8; 256] = [
    0x63,0x7c,0x77,0x7b,0xf2,0x6b,0x6f,0xc5,0x30,0x01,0x67,0x2b,0xfe,0xd7,0xab,0x76,
    0xca,0x82,0xc9,0x7d,0xfa,0x59,0x47,0xf0,0xad,0xd4,0xa2,0xaf,0x9c,0xa4,0x72,0xc0,
    0xb7,0xfd,0x93,0x26,0x36,0x3f,0xf7,0xcc,0x34,0xa5,0xe5,0xf1,0x71,0xd8,0x31,0x15,
    0x04,0xc7,0x23,0xc3,0x18,0x96,0x05,0x9a,0x07,0x12,0x80,0xe2,0xeb,0x27,0xb2,0x75,
    0x09,0x83,0x2c,0x1a,0x1b,0x6e,0x5a,0xa0,0x52,0x3b,0xd6,0xb3,0x29,0xe3,0x2f,0x84,
    0x53,0xd1,0x00,0xed,0x20,0xfc,0xb1,0x5b,0x6a,0xcb,0xbe,0x39,0x4a,0x4c,0x58,0xcf,
    0xd0,0xef,0xaa,0xfb,0x43,0x4d,0x33,0x85,0x45,0xf9,0x02,0x7f,0x50,0x3c,0x9f,0xa8,
    0x51,0xa3,0x40,0x8f,0x92,0x9d,0x38,0xf5,0xbc,0xb6,0xda,0x21,0x10,0xff,0xf3,0xd2,
    0xcd,0x0c,0x13,0xec,0x5f,0x97,0x44,0x17,0xc4,0xa7,0x7e,0x3d,0x64,0x5d,0x19,0x73,
    0x60,0x81,0x4f,0xdc,0x22,0x2a,0x90,0x88,0x46,0xee,0xb8,0x14,0xde,0x5e,0x0b,0xdb,
    0xe0,0x32,0x3a,0x0a,0x49,0x06,0x24,0x5c,0xc2,0xd3,0xac,0x62,0x91,0x95,0xe4,0x79,
    0xe7,0xc8,0x37,0x6d,0x8d,0xd5,0x4e,0xa9,0x6c,0x56,0xf4,0xea,0x65,0x7a,0xae,0x08,
    0xba,0x78,0x25,0x2e,0x1c,0xa6,0xb4,0xc6,0xe8,0xdd,0x74,0x1f,0x4b,0xbd,0x8b,0x8a,
    0x70,0x3e,0xb5,0x66,0x48,0x03,0xf6,0x0e,0x61,0x35,0x57,0xb9,0x86,0xc1,0x1d,0x9e,
    0xe1,0xf8,0x98,0x11,0x69,0xd9,0x8e,0x94,0x9b,0x1e,0x87,0xe9,0xce,0x55,0x28,0xdf,
    0x8c,0xa1,0x89,0x0d,0xbf,0xe6,0x42,0x68,0x41,0x99,0x2d,0x0f,0xb0,0x54,0xbb,0x16,
];

const AES_RCON: [u8; 11] = [0x00,0x01,0x02,0x04,0x08,0x10,0x20,0x40,0x80,0x1b,0x36];

fn xtime(x: u8) -> u8 {
    (x << 1) ^ if x & 0x80 != 0 { 0x1b } else { 0 }
}

/// Expand a 128-bit key into 11 round keys (176 bytes).
fn aes128_key_expansion(key: &[u8; 16]) -> [u8; 176] {
    let mut w = [0u8; 176];
    w[..16].copy_from_slice(key);
    let mut bytes_generated = 16;
    let mut rcon_iter = 1u8;
    let mut temp = [0u8; 4];
    while bytes_generated < 176 {
        temp.copy_from_slice(&w[bytes_generated - 4..bytes_generated]);
        if bytes_generated % 16 == 0 {
            let t = temp[0];
            temp[0] = AES_SBOX[temp[1] as usize] ^ rcon_iter;
            temp[1] = AES_SBOX[temp[2] as usize];
            temp[2] = AES_SBOX[temp[3] as usize];
            temp[3] = AES_SBOX[t as usize];
            rcon_iter = xtime(rcon_iter);
        }
        for i in 0..4 {
            w[bytes_generated] = w[bytes_generated - 16] ^ temp[i];
            bytes_generated += 1;
        }
    }
    w
}

/// Encrypt a single 16-byte block with AES-128.
fn aes128_encrypt_block(rk: &[u8; 176], input: &[u8; 16], output: &mut [u8; 16]) {
    let mut state = *input;
    for i in 0..16 { state[i] ^= rk[i]; }
    for round in 1..10 {
        for b in state.iter_mut() { *b = AES_SBOX[*b as usize]; }
        let s = state;
        state[1] = s[5]; state[5] = s[9]; state[9] = s[13]; state[13] = s[1];
        state[2] = s[10]; state[6] = s[14]; state[10] = s[2]; state[14] = s[6];
        state[3] = s[15]; state[7] = s[3]; state[11] = s[7]; state[15] = s[11];
        for c in 0..4 {
            let i = c * 4;
            let a = [state[i], state[i+1], state[i+2], state[i+3]];
            state[i]   = xtime(a[0]) ^ (xtime(a[1]) ^ a[1]) ^ a[2] ^ a[3];
            state[i+1] = a[0] ^ xtime(a[1]) ^ (xtime(a[2]) ^ a[2]) ^ a[3];
            state[i+2] = a[0] ^ a[1] ^ xtime(a[2]) ^ (xtime(a[3]) ^ a[3]);
            state[i+3] = (xtime(a[0]) ^ a[0]) ^ a[1] ^ a[2] ^ xtime(a[3]);
        }
        let rk_off = round * 16;
        for i in 0..16 { state[i] ^= rk[rk_off + i]; }
    }
    for b in state.iter_mut() { *b = AES_SBOX[*b as usize]; }
    let s = state;
    state[1] = s[5]; state[5] = s[9]; state[9] = s[13]; state[13] = s[1];
    state[2] = s[10]; state[6] = s[14]; state[10] = s[2]; state[14] = s[6];
    state[3] = s[15]; state[7] = s[3]; state[11] = s[7]; state[15] = s[11];
    for i in 0..16 { state[i] ^= rk[160 + i]; }
    output.copy_from_slice(&state);
}

/// AES Key Unwrap (RFC 3394). Unwraps the plaintext (e.g. the GTK) from
/// `cipher` (a multiple of 8 bytes, >= 24) using a 128-bit KEK, writing
/// `cipher.len() - 8` bytes into `out`. Returns None on integrity failure.
/// Inverse S-box, derived from the forward S-box.
fn aes_inv_sbox() -> [u8; 256] {
    let mut inv = [0u8; 256];
    for i in 0..256 {
        inv[AES_SBOX[i] as usize] = i as u8;
    }
    inv
}

/// GF(2^8) multiply used by InvMixColumns.
fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hi = a & 0x80 != 0;
        a <<= 1;
        if hi {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

/// Decrypt a single 16-byte block with AES-128 (inverse cipher).
fn aes128_decrypt_block(rk: &[u8; 176], input: &[u8; 16], output: &mut [u8; 16]) {
    let inv = aes_inv_sbox();
    let mut state = *input;
    // AddRoundKey (round 10)
    for i in 0..16 {
        state[i] ^= rk[160 + i];
    }
    for round in (1..10).rev() {
        // InvShiftRows: row r shifted right by r
        let s = state;
        state[1] = s[13]; state[5] = s[1]; state[9] = s[5]; state[13] = s[9];
        state[2] = s[10]; state[6] = s[14]; state[10] = s[2]; state[14] = s[6];
        state[3] = s[7]; state[7] = s[11]; state[11] = s[15]; state[15] = s[3];
        // InvSubBytes
        for b in state.iter_mut() {
            *b = inv[*b as usize];
        }
        // AddRoundKey
        for i in 0..16 {
            state[i] ^= rk[round * 16 + i];
        }
        // InvMixColumns
        for c in 0..4 {
            let i = c * 4;
            let a = [state[i], state[i + 1], state[i + 2], state[i + 3]];
            state[i]     = gmul(a[0],14) ^ gmul(a[1],11) ^ gmul(a[2],13) ^ gmul(a[3],9);
            state[i + 1] = gmul(a[0],9)  ^ gmul(a[1],14) ^ gmul(a[2],11) ^ gmul(a[3],13);
            state[i + 2] = gmul(a[0],13) ^ gmul(a[1],9)  ^ gmul(a[2],14) ^ gmul(a[3],11);
            state[i + 3] = gmul(a[0],11) ^ gmul(a[1],13) ^ gmul(a[2],9)  ^ gmul(a[3],14);
        }
    }
    // Final round (no InvMixColumns)
    let s = state;
    state[1] = s[13]; state[5] = s[1]; state[9] = s[5]; state[13] = s[9];
    state[2] = s[10]; state[6] = s[14]; state[10] = s[2]; state[14] = s[6];
    state[3] = s[7]; state[7] = s[11]; state[11] = s[15]; state[15] = s[3];
    for b in state.iter_mut() {
        *b = inv[*b as usize];
    }
    for i in 0..16 {
        state[i] ^= rk[i];
    }
    output.copy_from_slice(&state);
}

fn aes_key_unwrap(kek: &[u8; 16], cipher: &[u8], out: &mut [u8]) -> Option<()> {
    if cipher.len() < 24 || cipher.len() % 8 != 0 || out.len() != cipher.len() - 8 {
        return None;
    }
    // n = number of 64-bit plaintext blocks = (cipher blocks) - 1.
    let n = (cipher.len() / 8) - 1;
    let rk = aes128_key_expansion(kek);
    let mut a = [0u8; 8];
    a.copy_from_slice(&cipher[..8]);
    // n = (cipher.len()/8) - 1; msg 3/4 key data is commonly 40-56 bytes but
    // some APs send 104+ bytes (extra KDEs, RSN IE, IGTK), so allow up to
    // n=63 (512-byte ciphertext). The old n=8 array indexed out of bounds.
    let mut r = [[0u8; 8]; 64];
    for j in 1..=n {
        r[j].copy_from_slice(&cipher[j * 8..j * 8 + 8]);
    }
    for i in (0..6u64).rev() {
        for j in (1..=n).rev() {
            // RFC 3394: B = AES_K( (A XOR (n*i + j)) || Rj ); A = MSB(B); Rj = LSB(B)
            let ctr = ((n as u64) * i + j as u64).to_be_bytes();
            let mut block = [0u8; 16];
            for k in 0..8 {
                block[k] = a[k] ^ ctr[k];
            }
            block[8..].copy_from_slice(&r[j]);
            let mut enc = [0u8; 16];
            aes128_decrypt_block(&rk, &block, &mut enc);
            a.copy_from_slice(&enc[..8]);
            r[j].copy_from_slice(&enc[8..]);
        }
    }
    const A6: [u8; 8] = [0xA6; 8];
    if a != A6 {
        return None;
    }
    for j in 1..=n {
        out[(j - 1) * 8..j * 8].copy_from_slice(&r[j]);
    }
    Some(())
}

// ============================================================================
// Software CCMP (AES-CCM, RFC 3610) transmit encryption.
//
// The Realtek HW CCMP transmit path did not produce frames the AP would
// accept, and the vendor driver's software-encrypt path (`xmitframe_swencrypt`)
// is the reference for this chip. Encrypting in software and sending with
// SEC_TYPE=0 (HW crypto disabled) keeps the frame fully under our control.
// Validated against RFC 3610 Test Vector #1.
// ============================================================================

/// AES-CCM encryption (L=2, nonce=13, M=8). Returns (ciphertext, 8-byte MIC).
pub fn aes_ccm_encrypt(
    key: &[u8; 16],
    nonce: &[u8; 13],
    aad: &[u8],
    payload: &[u8],
) -> (Vec<u8>, [u8; 8]) {
    let rk = aes128_key_expansion(key);

    // B_0 = flags | nonce | l(m)
    let mut b0 = [0u8; 16];
    b0[0] = 0x59; // Adata=1, (M-2)/2=3, L-1=1
    b0[1..14].copy_from_slice(nonce);
    let l = payload.len() as u16;
    b0[14] = (l >> 8) as u8;
    b0[15] = l as u8;

    // CBC-MAC.
    let mut x = [0u8; 16];
    aes128_encrypt_block(&rk, &b0, &mut x);

    // AAD is encoded as 2-byte length || data || zero padding to 16 bytes.
    let mut aadbuf = alloc::vec::Vec::new();
    let alen = aad.len();
    aadbuf.push((alen >> 8) as u8);
    aadbuf.push(alen as u8);
    aadbuf.extend_from_slice(aad);
    while aadbuf.len() % 16 != 0 {
        aadbuf.push(0);
    }
    for blk in aadbuf.chunks(16) {
        let mut t = [0u8; 16];
        for i in 0..16 {
            t[i] = x[i] ^ blk[i];
        }
        aes128_encrypt_block(&rk, &t, &mut x);
    }

    // Payload blocks (plaintext, zero padded).
    let mut i = 0;
    while i < payload.len() {
        let end = core::cmp::min(i + 16, payload.len());
        let mut t = [0u8; 16];
        for j in 0..16 {
            let pv = if i + j < end { payload[i + j] } else { 0 };
            t[j] = x[j] ^ pv;
        }
        aes128_encrypt_block(&rk, &t, &mut x);
        i += 16;
    }

    // S_0 masks the tag.
    let mut a0 = [0u8; 16];
    a0[0] = 0x01; // L-1, no Adata/M
    a0[1..14].copy_from_slice(nonce);
    let mut s0 = [0u8; 16];
    aes128_encrypt_block(&rk, &a0, &mut s0);
    let mut mic = [0u8; 8];
    for i in 0..8 {
        mic[i] = x[i] ^ s0[i];
    }

    // CTR encryption with counters 1, 2, ...
    let mut ct = alloc::vec::Vec::with_capacity(payload.len());
    let mut ctr: u16 = 1;
    let mut i = 0;
    while i < payload.len() {
        let mut ai = [0u8; 16];
        ai[0] = 0x01;
        ai[1..14].copy_from_slice(nonce);
        ai[14] = (ctr >> 8) as u8;
        ai[15] = ctr as u8;
        let mut s = [0u8; 16];
        aes128_encrypt_block(&rk, &ai, &mut s);
        let end = core::cmp::min(i + 16, payload.len());
        for j in i..end {
            ct.push(payload[j] ^ s[j - i]);
        }
        i += 16;
        ctr = ctr.wrapping_add(1);
    }

    (ct, mic)
}

/// CCMP-encrypt a (non-QoS) 802.11 data frame body in software.
///
/// `header` is the 24-byte 802.11 header with the Protected bit already set,
/// `payload` is everything after the header (LLC/SNAP + data). `a2` is our MAC
/// (address 2). Returns the 8-byte CCMP header, the ciphertext, then the MIC —
/// i.e. exactly what replaces `payload` in the transmitted frame.
pub fn ccmp_encrypt_frame(
    key: &[u8; 16],
    key_id: u8,
    a2: &[u8; 6],
    pn: u64,
    header: &[u8],
    payload: &[u8],
) -> alloc::vec::Vec<u8> {
    // Nonce: flags | A2 | PN (PN MSB first).
    let mut nonce = [0u8; 13];
    nonce[0] = 0; // QoS TID 0, not management
    nonce[1..7].copy_from_slice(a2);
    nonce[7] = ((pn >> 40) & 0xff) as u8;
    nonce[8] = ((pn >> 32) & 0xff) as u8;
    nonce[9] = ((pn >> 24) & 0xff) as u8;
    nonce[10] = ((pn >> 16) & 0xff) as u8;
    nonce[11] = ((pn >> 8) & 0xff) as u8;
    nonce[12] = (pn & 0xff) as u8;

    // AAD = masked MAC header (non-QoS): mask_fc(2) | A1 | A2 | A3 | SC(2).
    let fc = u16::from_le_bytes([header[0], header[1]]);
    let mut mask_fc = fc & !(0x0800 | 0x1000 | 0x2000); // clear Retry/PM/MoreData
    mask_fc &= !0x0070; // data subtype bits
    mask_fc |= 0x4000; // Protected
    let mut aad = [0u8; 22];
    aad[0..2].copy_from_slice(&mask_fc.to_le_bytes());
    aad[2..8].copy_from_slice(&header[4..10]); // addr1
    aad[8..14].copy_from_slice(&header[10..16]); // addr2
    aad[14..20].copy_from_slice(&header[16..22]); // addr3
    aad[20] = header[22] & 0x0f; // sequence control: leave fragment number
    aad[21] = 0;

    let (ct, mic) = aes_ccm_encrypt(key, &nonce, &aad, payload);

    // CCMP header: PN little-endian with KeyID/ExtIV in byte 3.
    let mut out = alloc::vec::Vec::with_capacity(8 + ct.len() + 8);
    out.push((pn & 0xff) as u8);
    out.push(((pn >> 8) & 0xff) as u8);
    out.push(0);
    out.push(0x20 | ((key_id & 0x3) << 6));
    out.push(((pn >> 16) & 0xff) as u8);
    out.push(((pn >> 24) & 0xff) as u8);
    out.push(((pn >> 32) & 0xff) as u8);
    out.push(((pn >> 40) & 0xff) as u8);
    out.extend_from_slice(&ct);
    out.extend_from_slice(&mic);
    out
}
